//! The users API: who the request is from.

use axum::Json;

use app_core::users::User;

use crate::current_user::CurrentUser;

pub async fn me(CurrentUser(user): CurrentUser) -> Json<User> {
    Json(user)
}
