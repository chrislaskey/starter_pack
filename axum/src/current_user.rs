//! Who a request is from: the user its `Authorization: Bearer` token
//! stands for, which is how the apps call the API, see
//! `app_core::users::get_by_token`; a token the server does not know is
//! answered 401. A request with no token, the pages in a browser today,
//! is answered as the dev user until there is a login: this is the one
//! place on the server the login replaces, see `app_core::users`.

use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;

use app_core::deployment::Deployment;
use app_core::users::{self, User};

use crate::api_error::ApiError;

/// The request's user, an extractor for every handler.
pub struct CurrentUser(pub User);

impl FromRequestParts<Deployment> for CurrentUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        deployment: &Deployment,
    ) -> Result<CurrentUser, ApiError> {
        let header = parts
            .headers
            .get(AUTHORIZATION)
            .map(|value| value.to_str().unwrap_or_default());
        let user = match header {
            Some(header) => {
                let token = header.strip_prefix("Bearer ").ok_or_else(|| {
                    ApiError::new(StatusCode::UNAUTHORIZED, "a bearer token is needed")
                })?;
                users::get_by_token(deployment, token.trim()).await?
            }
            None => users::get(deployment, users::DEV_USER_ID).await?,
        };
        Ok(CurrentUser(user))
    }
}
