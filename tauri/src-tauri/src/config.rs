//! Where the Phoenix server is.
//!
//! The server URL comes from the `PHOENIX_URL` environment variable and
//! defaults to the Phoenix dev server. The websocket API and the web UI are
//! both derived from it.

use std::net::SocketAddr;

use url::Url;

const DEFAULT_SERVER_URL: &str = "http://localhost:4000";

#[derive(Clone, Debug)]
pub struct Config {
    pub server_url: Url,
}

impl Config {
    pub fn from_env() -> Self {
        let raw = std::env::var("PHOENIX_URL").unwrap_or_else(|_| DEFAULT_SERVER_URL.to_string());
        let server_url = Url::parse(&raw)
            .unwrap_or_else(|error| panic!("PHOENIX_URL {raw:?} is not a valid URL: {error}"));

        Self { server_url }
    }

    /// The websocket API, `App.Web.ApiSocket`, mounted at `/api`.
    pub fn websocket_url(&self) -> Url {
        let mut url = self.server_url.clone();
        let scheme = if url.scheme() == "https" { "wss" } else { "ws" };
        url.set_scheme(scheme)
            .expect("http(s) to ws(s) is always allowed");
        url.set_path("/api/websocket");
        url.set_query(Some("vsn=2.0.0"));
        url
    }

    /// A page of the web UI.
    pub fn page_url(&self, path: &str) -> Result<Url, String> {
        self.server_url
            .join(path)
            .map_err(|error| error.to_string())
    }

    /// A sync endpoint of a realm, `App.Web.SyncController`: the realm's
    /// host on the server's scheme and port.
    pub fn sync_url(&self, realm_host: &str, path: &str) -> Result<Url, String> {
        let mut url = self.server_url.clone();
        url.set_host(Some(realm_host))
            .map_err(|error| format!("{realm_host} is not a valid host: {error}"))?;
        url.set_path(path);
        url.set_query(None);
        Ok(url)
    }

    /// Where a realm host resolves to in development. Browsers resolve
    /// `*.localhost` to this machine but the system resolver does not, so
    /// such a host points at the server's address.
    pub fn dev_address(&self, realm_host: &str) -> Option<SocketAddr> {
        if !realm_host.ends_with(".localhost") {
            return None;
        }
        let port = self.server_url.port_or_known_default()?;
        Some(SocketAddr::from(([127, 0, 0, 1], port)))
    }
}
