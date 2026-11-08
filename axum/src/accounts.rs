//! The accounts API: sign up, sign in and sign out, see
//! `app_core::accounts`. A sign-up or a sign-in answers with the user
//! and the token, which the apps keep and send as a bearer token, and
//! sets the token as a cookie, which a browser sends from then on: only
//! HTTP sees it, only this site gets it, and only over TLS away from
//! localhost. A sign-out deletes the token and clears the cookie.

use axum::Json;
use axum::extract::State;
use axum::http::header::HOST;
use axum::http::{HeaderMap, StatusCode};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use serde::Deserialize;

use app_core::accounts::{self, Session};
use app_core::deployment::Deployment;

use crate::api_error::ApiError;
use crate::current_user::{CurrentToken, CurrentUser, TOKEN_COOKIE};

#[derive(Deserialize)]
pub struct Credentials {
    pub email: String,
    pub password: String,
}

pub async fn sign_up(
    State(deployment): State<Deployment>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(credentials): Json<Credentials>,
) -> Result<(StatusCode, CookieJar, Json<Session>), ApiError> {
    let session = accounts::sign_up(&deployment, &credentials.email, &credentials.password).await?;
    let jar = jar.add(token_cookie(&session.token, &headers));
    Ok((StatusCode::CREATED, jar, Json(session)))
}

pub async fn sign_in(
    State(deployment): State<Deployment>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(credentials): Json<Credentials>,
) -> Result<(CookieJar, Json<Session>), ApiError> {
    let session = accounts::sign_in(&deployment, &credentials.email, &credentials.password).await?;
    let jar = jar.add(token_cookie(&session.token, &headers));
    Ok((jar, Json(session)))
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

fn is_localhost(headers: &HeaderMap) -> bool {
    let host = headers
        .get(HOST)
        .and_then(|host| host.to_str().ok())
        .unwrap_or_default();
    let host = host.rsplit_once(':').map_or(host, |(name, _port)| name);
    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}
