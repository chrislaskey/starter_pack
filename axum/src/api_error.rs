//! An error as the API answers it: a status and `{"error": "..."}`, from
//! the error of whichever core module the handler called; a form's
//! fields that were wrong also as `"errors": {"field": ["message"]}`.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use app_core::accounts::AccountError;
use app_core::accounts::users::AccountUserError;
use app_core::avatars::AvatarError;
use app_core::communities::CommunityError;
use app_core::credentials::CredentialError;
use app_core::current_community::CurrentCommunityError;
use app_core::field_errors::FieldErrors;
use app_core::files::FileError;
use app_core::memberships::MembershipError;
use app_core::posts::PostError;
use app_core::posts_communities::PostCommunityError;
use app_core::sync::SyncError;
use app_core::users::UserError;

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
    field_errors: Option<FieldErrors>,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> ApiError {
        ApiError {
            status,
            message: message.into(),
            field_errors: None,
        }
    }

    /// 422, with the fields that were wrong.
    pub fn from_field_errors(field_errors: FieldErrors) -> ApiError {
        ApiError {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            message: "Some fields need another look".to_string(),
            field_errors: Some(field_errors),
        }
    }
}

impl From<PostError> for ApiError {
    fn from(error: PostError) -> Self {
        let status = match &error {
            PostError::NotFound { .. } => StatusCode::NOT_FOUND,
            PostError::NotOwner => StatusCode::FORBIDDEN,
            PostError::BlankBody | PostError::Invalid(_) | PostError::Rejected(_) => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            PostError::Community(error) => return ApiError::from_post_community(error),
            PostError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError::new(status, error.to_string())
    }
}

impl ApiError {
    fn from_post_community(error: &PostCommunityError) -> ApiError {
        let status = match error {
            PostCommunityError::UnknownPost { .. } | PostCommunityError::NotFound { .. } => {
                StatusCode::NOT_FOUND
            }
            PostCommunityError::NotOwner | PostCommunityError::NotAMember { .. } => {
                StatusCode::FORBIDDEN
            }
            PostCommunityError::Invalid(_) | PostCommunityError::Rejected(_) => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            PostCommunityError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError::new(status, error.to_string())
    }
}

impl From<PostCommunityError> for ApiError {
    fn from(error: PostCommunityError) -> Self {
        ApiError::from_post_community(&error)
    }
}

impl From<CommunityError> for ApiError {
    fn from(error: CommunityError) -> Self {
        let status = match &error {
            CommunityError::NotFound { .. } => StatusCode::NOT_FOUND,
            CommunityError::NotAMember => StatusCode::FORBIDDEN,
            CommunityError::Invalid(field_errors) => {
                return ApiError::from_field_errors(field_errors.clone());
            }
            CommunityError::Malformed(_) | CommunityError::Rejected(_) => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            CommunityError::Membership(error) => return ApiError::from_membership(error),
            CommunityError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError::new(status, error.to_string())
    }
}

impl ApiError {
    fn from_membership(error: &MembershipError) -> ApiError {
        let status = match error {
            MembershipError::NotFound { .. } | MembershipError::UnknownCommunity { .. } => {
                StatusCode::NOT_FOUND
            }
            MembershipError::Invalid(_) | MembershipError::Rejected(_) => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            MembershipError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError::new(status, error.to_string())
    }
}

impl From<MembershipError> for ApiError {
    fn from(error: MembershipError) -> Self {
        ApiError::from_membership(&error)
    }
}

impl From<CurrentCommunityError> for ApiError {
    fn from(error: CurrentCommunityError) -> Self {
        let status = match error {
            CurrentCommunityError::Community(error) => return error.into(),
            CurrentCommunityError::NotAMember => StatusCode::FORBIDDEN,
            CurrentCommunityError::DeviceOnly
            | CurrentCommunityError::DeviceSettings(_)
            | CurrentCommunityError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError::new(status, error.to_string())
    }
}

impl From<UserError> for ApiError {
    fn from(error: UserError) -> Self {
        let status = match error {
            UserError::NotFound { .. } => StatusCode::NOT_FOUND,
            UserError::UnknownToken | UserError::NotSignedIn => StatusCode::UNAUTHORIZED,
            UserError::InvalidName | UserError::Invalid(_) => StatusCode::UNPROCESSABLE_ENTITY,
            UserError::AccountUser(error) => return error.into(),
            UserError::ServerOnly | UserError::DeviceSettings(_) | UserError::Database(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        ApiError::new(status, error.to_string())
    }
}

impl From<AccountUserError> for ApiError {
    fn from(error: AccountUserError) -> Self {
        let status = match error {
            AccountUserError::NotFound { .. } => StatusCode::NOT_FOUND,
            AccountUserError::InvalidEmail | AccountUserError::EmailTaken => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            AccountUserError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError::new(status, error.to_string())
    }
}

impl From<AvatarError> for ApiError {
    fn from(error: AvatarError) -> Self {
        let status = match error {
            AvatarError::Invalid(field_errors) => return ApiError::from_field_errors(field_errors),
            AvatarError::User(error) => return error.into(),
            AvatarError::Sync(error) => return error.into(),
            AvatarError::NotYours => StatusCode::FORBIDDEN,
            AvatarError::NoAvatarBucket => StatusCode::SERVICE_UNAVAILABLE,
            AvatarError::UploadMissing
            | AvatarError::UploadExpired
            | AvatarError::NotTheImageType
            | AvatarError::TooLarge => StatusCode::UNPROCESSABLE_ENTITY,
            AvatarError::ServerOnly | AvatarError::Storage(_) | AvatarError::Database(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        ApiError::new(status, error.to_string())
    }
}

impl From<FileError> for ApiError {
    fn from(error: FileError) -> Self {
        let status = match error {
            FileError::Invalid(field_errors) => return ApiError::from_field_errors(field_errors),
            FileError::NotFound { .. } => StatusCode::NOT_FOUND,
            FileError::NotOwner => StatusCode::FORBIDDEN,
            FileError::NotUploaded | FileError::Rejected(_) | FileError::Malformed(_) => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            FileError::NoFileStore => StatusCode::SERVICE_UNAVAILABLE,
            FileError::Storage(_) => StatusCode::BAD_GATEWAY,
            FileError::Sync(error) => return (*error).into(),
            FileError::NotOnDevice => StatusCode::NOT_FOUND,
            FileError::DeviceOnly | FileError::Io(_) | FileError::Database(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        ApiError::new(status, error.to_string())
    }
}

impl From<AccountError> for ApiError {
    fn from(error: AccountError) -> Self {
        let status = match error {
            AccountError::User(error) => return error.into(),
            AccountError::AccountUser(error) => return error.into(),
            AccountError::Sync(error) => return error.into(),
            AccountError::InvalidEmailOrPassword
            | AccountError::CodeDoesNotMatch
            | AccountError::UnknownDeviceCode => StatusCode::UNAUTHORIZED,
            AccountError::Credential(
                CredentialError::PasswordTooShort | CredentialError::PasswordTooLong,
            ) => StatusCode::UNPROCESSABLE_ENTITY,
            AccountError::Mail(_) => StatusCode::SERVICE_UNAVAILABLE,
            AccountError::ServerOnly
            | AccountError::DeviceOnly
            | AccountError::Credential(_)
            | AccountError::DeviceSettings(_)
            | AccountError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError::new(status, error.to_string())
    }
}

impl From<SyncError> for ApiError {
    fn from(error: SyncError) -> Self {
        match error {
            SyncError::Post(error) => error.into(),
            SyncError::PostCommunity(error) => error.into(),
            SyncError::File(error) => error.into(),
            SyncError::Community(error) => error.into(),
            SyncError::Membership(error) => error.into(),
            SyncError::User(error) => error.into(),
            SyncError::Unauthorized => ApiError::new(StatusCode::UNAUTHORIZED, error.to_string()),
            SyncError::NotAMember { .. } => ApiError::new(StatusCode::FORBIDDEN, error.to_string()),
            SyncError::Transport(_) | SyncError::Refused(_) | SyncError::Malformed(_) => {
                ApiError::new(StatusCode::BAD_REQUEST, error.to_string())
            }
            SyncError::Database(_) | SyncError::DeviceSettings(_) | SyncError::Purge(_) => {
                ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if self.status.is_server_error() {
            tracing::error!("{}", self.message);
        }
        let body = match self.field_errors {
            Some(field_errors) => json!({ "error": self.message, "errors": field_errors }),
            None => json!({ "error": self.message }),
        };
        (self.status, Json(body)).into_response()
    }
}
