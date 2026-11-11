//! The accounts API: sign up, sign in, the code and a new one, and sign
//! out, see
//! `app_core::accounts` and `docs/design/accounts-sign-in.md`. A sign-up
//! or a sign-in answers 202 with the pending token, the code having been
//! emailed; the code answers the session and sets the token as a cookie,
//! which a browser sends from then on: only HTTP sees it, only this site
//! gets it, and only over TLS away from localhost. The code with a
//! device's challenge answers the device code instead and sets no
//! cookie; the device exchanges it with its verifier for its session,
//! which it keeps and sends as a bearer token. A sign-out deletes the
//! token and clears the cookie. Each route is rate limited, see
//! `rate_limits`.

use axum::Json;
use axum::extract::State;
use axum::http::header::HOST;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use serde::Deserialize;

use app_core::accounts::{self, CodeSent, Session};
use app_core::deployment::Deployment;
use app_core::mailer::Mailer;

use crate::api_error::ApiError;
use crate::current_user::{CurrentToken, CurrentUser, TOKEN_COOKIE};
use crate::rate_limits::RateLimits;

#[derive(Deserialize)]
pub struct Credentials {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct Code {
    pub pending: String,
    pub code: String,
    /// A device's challenge, when the sign-in was started by one.
    pub challenge: Option<String>,
}

#[derive(Deserialize)]
pub struct DeviceCodeExchange {
    pub code: String,
    pub verifier: String,
}

#[derive(Deserialize)]
pub struct SignUp {
    pub email: String,
    pub password: String,
    /// What the pages show the person by.
    pub name: String,
}

pub async fn sign_up(
    State(deployment): State<Deployment>,
    State(mailer): State<Mailer>,
    State(limits): State<RateLimits>,
    Json(sign_up): Json<SignUp>,
) -> Result<(StatusCode, Json<CodeSent>), ApiError> {
    limits.check_email(&sign_up.email)?;
    let sent = accounts::sign_up(
        &deployment,
        &mailer,
        &sign_up.email,
        &sign_up.password,
        &sign_up.name,
    )
    .await?;
    Ok((StatusCode::ACCEPTED, Json(sent)))
}

pub async fn sign_in(
    State(deployment): State<Deployment>,
    State(mailer): State<Mailer>,
    State(limits): State<RateLimits>,
    Json(credentials): Json<Credentials>,
) -> Result<(StatusCode, Json<CodeSent>), ApiError> {
    limits.check_email(&credentials.email)?;
    let sent = accounts::sign_in(
        &deployment,
        &mailer,
        &credentials.email,
        &credentials.password,
    )
    .await?;
    Ok((StatusCode::ACCEPTED, Json(sent)))
}

#[derive(Deserialize)]
pub struct Resend {
    pub pending: String,
}

/// A new code emailed for the pending sign-in.
pub async fn code_resend(
    State(deployment): State<Deployment>,
    State(mailer): State<Mailer>,
    State(limits): State<RateLimits>,
    Json(resend): Json<Resend>,
) -> Result<StatusCode, ApiError> {
    limits.check_resend(&resend.pending)?;
    let email = accounts::resend_code_to(&deployment, &resend.pending).await?;
    limits.check_email(&email)?;
    accounts::resend_code(&deployment, &mailer, &resend.pending).await?;
    Ok(StatusCode::ACCEPTED)
}

/// The emailed code: the session with the cookie, or, with a device's
/// challenge, the device code and no cookie.
pub async fn code(
    State(deployment): State<Deployment>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(code): Json<Code>,
) -> Result<Response, ApiError> {
    if let Some(challenge) = code.challenge.as_deref().filter(|c| !c.is_empty()) {
        let device_code = accounts::sign_in_with_code_for_device(
            &deployment,
            &code.pending,
            &code.code,
            challenge,
        )
        .await?;
        return Ok(Json(device_code).into_response());
    }
    let session = accounts::sign_in_with_code(&deployment, &code.pending, &code.code).await?;
    let jar = jar.add(token_cookie(&session.token, &headers));
    Ok((jar, Json(session)).into_response())
}

/// A device's exchange of its code and verifier for its session; no
/// cookie, the device keeps the token.
pub async fn device_token(
    State(deployment): State<Deployment>,
    Json(exchange): Json<DeviceCodeExchange>,
) -> Result<Json<Session>, ApiError> {
    let session =
        accounts::sign_in_with_device_code(&deployment, &exchange.code, &exchange.verifier).await?;
    Ok(Json(session))
}

pub async fn sign_out(
    State(deployment): State<Deployment>,
    CurrentUser(_user): CurrentUser,
    CurrentToken(token): CurrentToken,
    jar: CookieJar,
) -> Result<(StatusCode, CookieJar), ApiError> {
    accounts::sign_out(&deployment, &token).await?;
    let jar = jar.remove(Cookie::build(TOKEN_COOKIE).path("/"));
    Ok((StatusCode::NO_CONTENT, jar))
}

/// The cookie a browser keeps the token in, for this session of the
/// browser: `Secure` unless the request came to localhost, where there
/// is no TLS.
fn token_cookie(token: &str, headers: &HeaderMap) -> Cookie<'static> {
    Cookie::build((TOKEN_COOKIE, token.to_string()))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Strict)
        .secure(!is_localhost(headers))
        .build()
}

pub(crate) fn is_localhost(headers: &HeaderMap) -> bool {
    let host = headers
        .get(HOST)
        .and_then(|host| host.to_str().ok())
        .unwrap_or_default();
    let host = host.rsplit_once(':').map_or(host, |(name, _port)| name);
    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}
