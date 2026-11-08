//! Who a request is from: the user its token stands for, see
//! `app_core::users::get_by_token`. The apps send the token as
//! `Authorization: Bearer`; a browser sends the `token` cookie a sign-in
//! set, see `accounts`. A request with no token, or one the server does
//! not know, is answered 401, whatever it asks for.

use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum_extra::extract::CookieJar;

use app_core::deployment::Deployment;
use app_core::users::{self, User};

use crate::api_error::ApiError;

/// The cookie a browser keeps the token in.
pub const TOKEN_COOKIE: &str = "token";

/// The request's user, an extractor for every handler.
pub struct CurrentUser(pub User);

/// The token the request carried, for the sign-out to delete.
pub struct CurrentToken(pub String);

impl FromRequestParts<Deployment> for CurrentUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        deployment: &Deployment,
    ) -> Result<CurrentUser, ApiError> {
        let token = token_of(parts)?;
        let user = users::get_by_token(deployment, &token).await?;
        Ok(CurrentUser(user))
    }
}

impl FromRequestParts<Deployment> for CurrentToken {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        _deployment: &Deployment,
    ) -> Result<CurrentToken, ApiError> {
        token_of(parts).map(CurrentToken)
    }
}

/// The bearer token, else the cookie's.
fn token_of(parts: &Parts) -> Result<String, ApiError> {
    if let Some(header) = parts.headers.get(AUTHORIZATION) {
        let token = header
            .to_str()
            .ok()
            .and_then(|header| header.strip_prefix("Bearer "))
            .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "a bearer token is needed"))?;
        return Ok(token.trim().to_string());
    }
    CookieJar::from_headers(&parts.headers)
        .get(TOKEN_COOKIE)
        .map(|cookie| cookie.value().to_string())
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "Sign in first"))
}
