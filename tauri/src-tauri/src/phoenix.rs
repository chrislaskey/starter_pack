//! A client of the Phoenix websocket API, `App.Web.ApiSocket`.
//!
//! One task owns the connection for the life of the app. It connects with the
//! stored API token, joins the `"api"` channel, keeps the connection alive
//! with heartbeats and reconnects with a backoff when the connection drops.
//! Every change of state is emitted to the main window as a
//! `connection-status` event, and every message the server pushes as an
//! `api-message` event.
//!
//! Requests from the rest of the app arrive as [`Control`] messages. A
//! request is a Phoenix channel push on the `"api"` topic and its reply is
//! sent back on the request's oneshot channel.
//!
//! The wire format is the Phoenix channel protocol v2: every message is a
//! JSON array `[join_ref, ref, topic, event, payload]`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::{self, Message};

use crate::config::Config;
use crate::token::TokenStore;

const TOPIC: &str = "api";
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const MIN_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

pub type Reply = Result<Value, Value>;

/// What the rest of the app asks of the connection.
pub enum Control {
    /// The stored token changed: reconnect with the new one, or disconnect
    /// when there is none.
    TokenChanged,
    /// Push `event` with `payload` on the `"api"` channel and send the reply
    /// back on `reply`.
    Request {
        event: String,
        payload: Value,
        reply: oneshot::Sender<Reply>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// No connection: not logged in, waiting to reconnect or shut down.
    Disconnected,
    /// Opening the websocket.
    Connecting,
    /// The websocket is open and the channel join is in flight.
    Connected,
    /// The channel is joined and requests can be made.
    Joined,
}

#[derive(Clone, Debug, Serialize)]
pub struct Status {
    pub state: State,
    pub detail: Option<String>,
    pub has_token: bool,
    pub server_url: String,
}

/// The connection task. See the module docs.
pub struct Client {
    app: AppHandle,
    config: Config,
    tokens: Arc<TokenStore>,
    status: Arc<Mutex<Status>>,
    control: mpsc::UnboundedReceiver<Control>,
    joined_this_session: bool,
}

enum SessionEnd {
    TokenChanged,
    Unauthorized,
    Failed(String),
}

enum Idle {
    TokenChanged,
    Timeout,
    Closed,
}

impl Client {
    pub fn new(
        app: AppHandle,
        config: Config,
        tokens: Arc<TokenStore>,
        status: Arc<Mutex<Status>>,
        control: mpsc::UnboundedReceiver<Control>,
    ) -> Self {
        Self {
            app,
            config,
            tokens,
            status,
            control,
            joined_this_session: false,
        }
    }

    pub async fn run(mut self) {
        let mut backoff = MIN_BACKOFF;

        loop {
            let Some(token) = self.tokens.get() else {
                self.set_status(State::Disconnected, Some("Not logged in".into()));
                match self.idle(None).await {
                    Idle::Closed => return,
                    Idle::TokenChanged | Idle::Timeout => {}
                }
                backoff = MIN_BACKOFF;
                continue;
            };

            self.set_status(State::Connecting, None);
            self.joined_this_session = false;

            match self.session(&token).await {
                SessionEnd::TokenChanged => {
                    backoff = MIN_BACKOFF;
                }
                SessionEnd::Unauthorized => {
                    log::warn!("The server rejected the API token, dropping it");
                    if let Err(error) = self.tokens.clear() {
                        log::error!("Could not clear the API token: {error}");
                    }
                    backoff = MIN_BACKOFF;
                    self.set_status(
                        State::Disconnected,
                        Some("The server rejected the API token. Log in again.".into()),
                    );
                }
                SessionEnd::Failed(reason) => {
                    if self.joined_this_session {
                        backoff = MIN_BACKOFF;
                    }
                    log::info!("Disconnected: {reason}. Reconnecting in {backoff:?}");
                    self.set_status(
                        State::Disconnected,
                        Some(format!("{reason}. Reconnecting in {}s.", backoff.as_secs())),
                    );
                    match self.idle(Some(backoff)).await {
                        Idle::Closed => return,
                        Idle::TokenChanged => backoff = MIN_BACKOFF,
                        Idle::Timeout => backoff = (backoff * 2).min(MAX_BACKOFF),
                    }
                }
            }
        }
    }

    /// Waits for the token to change, or for `timeout` to pass. Requests
    /// that arrive meanwhile are refused.
    async fn idle(&mut self, timeout: Option<Duration>) -> Idle {
        let deadline = tokio::time::sleep(timeout.unwrap_or(Duration::MAX));
        tokio::pin!(deadline);

        loop {
            tokio::select! {
                control = self.control.recv() => match control {
                    None => return Idle::Closed,
                    Some(Control::TokenChanged) => return Idle::TokenChanged,
                    Some(Control::Request { reply, .. }) => {
                        let _ = reply.send(Err(json!({"reason": "not_connected"})));
                    }
                },
                _ = &mut deadline, if timeout.is_some() => return Idle::Timeout,
            }
        }
    }

    /// One connection: from the websocket handshake until the connection ends.
    async fn session(&mut self, token: &str) -> SessionEnd {
        let request = match self.connect_request(token) {
            Ok(request) => request,
            Err(error) => return SessionEnd::Failed(error),
        };

        log::info!("Connecting to {}", self.config.websocket_url());

        let (websocket, _response) = match tokio_tungstenite::connect_async(request).await {
            Ok(connection) => connection,
            Err(tungstenite::Error::Http(response)) if response.status() == 403 => {
                return SessionEnd::Unauthorized;
            }
            Err(error) => return SessionEnd::Failed(format!("Could not connect: {error}")),
        };

        self.set_status(State::Connected, Some("Joining the api channel".into()));

        let (mut sink, mut stream) = websocket.split();
        let mut next_ref: u64 = 0;
        let mut make_ref = move || {
            next_ref += 1;
            next_ref.to_string()
        };

        let join_ref = make_ref();
        if let Err(error) = send(
            &mut sink,
            &json!([join_ref, join_ref, TOPIC, "phx_join", {}]),
        )
        .await
        {
            return SessionEnd::Failed(error);
        }

        let mut pending: HashMap<String, oneshot::Sender<Reply>> = HashMap::new();
        let mut heartbeat_ref: Option<String> = None;
        let mut heartbeat = tokio::time::interval_at(
            tokio::time::Instant::now() + HEARTBEAT_INTERVAL,
            HEARTBEAT_INTERVAL,
        );
        let mut joined = false;

        loop {
            tokio::select! {
                message = stream.next() => {
                    let message = match message {
                        None => return SessionEnd::Failed("The connection closed".into()),
                        Some(Err(error)) => return SessionEnd::Failed(format!("Connection error: {error}")),
                        Some(Ok(message)) => message,
                    };

                    match message {
                        Message::Text(text) => {
                            let Some((reference, event, payload)) = parse(text.as_str()) else {
                                log::warn!("Ignoring a message that is not a Phoenix v2 message: {text}");
                                continue;
                            };

                            match event.as_str() {
                                "phx_reply" if heartbeat_ref.as_deref() == Some(reference.as_str()) => {
                                    heartbeat_ref = None;
                                }
                                "phx_reply" if reference == join_ref => {
                                    if reply_status(&payload) == "ok" {
                                        joined = true;
                                        self.joined_this_session = true;
                                        log::info!("Joined the {TOPIC} channel");
                                        self.set_status(State::Joined, None);
                                    } else {
                                        return SessionEnd::Failed(format!("Join refused: {}", reply_response(payload)));
                                    }
                                }
                                "phx_reply" => {
                                    if let Some(reply) = pending.remove(&reference) {
                                        let response = reply_response(payload.clone());
                                        let result = if reply_status(&payload) == "ok" { Ok(response) } else { Err(response) };
                                        let _ = reply.send(result);
                                    }
                                }
                                "phx_error" => return SessionEnd::Failed("The channel crashed".into()),
                                "phx_close" => return SessionEnd::Failed("The channel closed".into()),
                                _ => {
                                    // A message the server pushed on its own.
                                    let _ = self.app.emit_to("main", "api-message", json!({"event": event, "payload": payload}));
                                }
                            }
                        }
                        Message::Ping(data) => {
                            if let Err(error) = sink.send(Message::Pong(data)).await {
                                return SessionEnd::Failed(format!("Could not answer a ping: {error}"));
                            }
                        }
                        Message::Close(frame) => {
                            let reason = frame
                                .map(|frame| format!(" ({} {})", frame.code, frame.reason))
                                .unwrap_or_default();
                            return SessionEnd::Failed(format!("The server closed the connection{reason}"));
                        }
                        _ => {}
                    }
                }

                control = self.control.recv() => match control {
                    None => {
                        let _ = sink.send(Message::Close(None)).await;
                        return SessionEnd::Failed("Shutting down".into());
                    }
                    Some(Control::TokenChanged) => {
                        let _ = sink.send(Message::Close(None)).await;
                        return SessionEnd::TokenChanged;
                    }
                    Some(Control::Request { event, payload, reply }) => {
                        if !joined {
                            let _ = reply.send(Err(json!({"reason": "not_connected"})));
                            continue;
                        }
                        let reference = make_ref();
                        if let Err(error) = send(&mut sink, &json!([join_ref, reference, TOPIC, event, payload])).await {
                            let _ = reply.send(Err(json!({"reason": "send_failed", "error": error})));
                            return SessionEnd::Failed(error);
                        }
                        pending.insert(reference, reply);
                    }
                },

                _ = heartbeat.tick() => {
                    if heartbeat_ref.is_some() {
                        return SessionEnd::Failed("The heartbeat timed out".into());
                    }
                    let reference = make_ref();
                    if let Err(error) = send(&mut sink, &json!([null, reference, "phoenix", "heartbeat", {}])).await {
                        return SessionEnd::Failed(error);
                    }
                    heartbeat_ref = Some(reference);
                }
            }
        }
    }

    /// The websocket upgrade request. The token goes in the
    /// `Sec-WebSocket-Protocol` header the way `App.Web.ApiSocket` expects,
    /// which is also how the `phoenix` JavaScript client sends it.
    fn connect_request(
        &self,
        token: &str,
    ) -> Result<tungstenite::handshake::client::Request, String> {
        let mut request = self
            .config
            .websocket_url()
            .as_str()
            .into_client_request()
            .map_err(|error| error.to_string())?;

        let encoded = base64::engine::general_purpose::STANDARD_NO_PAD.encode(token);
        let protocols = format!("phoenix, base64url.bearer.phx.{encoded}");
        request.headers_mut().insert(
            "Sec-WebSocket-Protocol",
            HeaderValue::from_str(&protocols).map_err(|error| error.to_string())?,
        );

        Ok(request)
    }

    fn set_status(&self, state: State, detail: Option<String>) {
        let status = {
            let mut status = self.status.lock().unwrap();
            status.state = state;
            status.detail = detail;
            status.has_token = self.tokens.get().is_some();
            status.clone()
        };

        if let Err(error) = self.app.emit_to("main", "connection-status", &status) {
            log::warn!("Could not emit the connection status: {error}");
        }
    }
}

async fn send<S>(sink: &mut S, message: &Value) -> Result<(), String>
where
    S: SinkExt<Message> + Unpin,
    S::Error: std::fmt::Display,
{
    sink.send(Message::text(message.to_string()))
        .await
        .map_err(|error| format!("Could not send: {error}"))
}

/// Splits a v2 message into its ref, event and payload. The ref is `""` for
/// a message the server pushed on its own.
fn parse(text: &str) -> Option<(String, String, Value)> {
    let message: Vec<Value> = serde_json::from_str(text).ok()?;
    let [_join_ref, reference, _topic, event, payload] = message.try_into().ok()?;

    Some((
        reference.as_str().unwrap_or_default().to_string(),
        event.as_str()?.to_string(),
        payload,
    ))
}

fn reply_status(payload: &Value) -> &str {
    payload
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("error")
}

fn reply_response(mut payload: Value) -> Value {
    payload
        .get_mut("response")
        .map(Value::take)
        .unwrap_or(Value::Null)
}
