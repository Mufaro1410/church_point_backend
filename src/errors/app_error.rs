use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

use super::domain::{
    announcement::AnnouncementError, auth::AuthError, event::EventError, finance::FinanceError, foc::FocError,
    member::MemberError, org::OrgError, prayer::PrayerError, sermon::SermonError, testimony::TestimonyError,
};

pub type AppResult<T> = Result<T, AppError>;

/// The one error type every handler/service returns.
/// Domain error -> AppError -> HTTP response.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Auth(#[from] AuthError),
    #[error(transparent)]
    Member(#[from] MemberError),
    #[error(transparent)]
    Foc(#[from] FocError),
    #[error(transparent)]
    Finance(#[from] FinanceError),
    #[error(transparent)]
    Event(#[from] EventError),
    #[error(transparent)]
    Sermon(#[from] SermonError),
    #[error(transparent)]
    Org(#[from] OrgError),
    #[error(transparent)]
    Prayer(#[from] PrayerError),
    #[error(transparent)]
    Testimony(#[from] TestimonyError),
    #[error(transparent)]
    Announcement(#[from] AnnouncementError),
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Upstream(String),
    #[error("{0} is not configured on the server.")]
    NotConfigured(&'static str),
    #[error("Database error")]
    Database(#[from] sqlx::Error),
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Some database errors are the CLIENT's fault (bad/missing value, duplicate, linked rows).
/// Turn those into 4xx responses instead of a generic 500.
fn client_db_error(e: &sqlx::Error) -> Option<(StatusCode, &'static str, &'static str)> {
    let code = e.as_database_error()?.code()?;
    let code = code.as_ref();
    match code {
        "23505" => Some((StatusCode::CONFLICT, "DUPLICATE_VALUE", "A record with these values already exists.")),
        "23503" => Some((StatusCode::CONFLICT, "LINKED_RECORDS", "This record is linked to other records.")),
        "23502" => Some((StatusCode::UNPROCESSABLE_ENTITY, "MISSING_FIELD", "A required field is missing.")),
        "23514" => Some((StatusCode::UNPROCESSABLE_ENTITY, "INVALID_VALUE", "A value is not allowed.")),
        c if c.starts_with("22") => {
            Some((StatusCode::UNPROCESSABLE_ENTITY, "INVALID_VALUE", "A value has the wrong format."))
        }
        _ => None,
    }
}

impl AppError {
    fn parts(&self) -> (StatusCode, &'static str, Option<&'static str>) {
        match self {
            AppError::Auth(e) => (e.status(), e.code(), None),
            AppError::Member(e) => (e.status(), e.code(), None),
            AppError::Foc(e) => (e.status(), e.code(), None),
            AppError::Finance(e) => (e.status(), e.code(), None),
            AppError::Event(e) => (e.status(), e.code(), None),
            AppError::Sermon(e) => (e.status(), e.code(), None),
            AppError::Org(e) => (e.status(), e.code(), None),
            AppError::Prayer(e) => (e.status(), e.code(), None),
            AppError::Testimony(e) => (e.status(), e.code(), None),
            AppError::Announcement(e) => (e.status(), e.code(), None),
            AppError::Validation(_) => (StatusCode::UNPROCESSABLE_ENTITY, "VALIDATION_ERROR", None),
            AppError::Database(e) => match client_db_error(e) {
                Some((s, c, m)) => (s, c, Some(m)),
                None => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", None),
            },
            AppError::Conflict(_) => (StatusCode::CONFLICT, "REQUEST_IN_PROGRESS", None),
            AppError::Upstream(_) => (StatusCode::BAD_GATEWAY, "UPSTREAM_ERROR", Some("A supporting service is not responding. Try again.")),
            AppError::NotConfigured(_) => (StatusCode::SERVICE_UNAVAILABLE, "NOT_CONFIGURED", None),
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", None),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, fixed_message) = self.parts();
        let message = if status == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!(error = ?self, "unexpected error"); // log cause, hide from client
            "An unexpected error occurred.".to_string()
        } else {
            if let AppError::Database(e) = &self {
                tracing::warn!(error = ?e, "client database error");
            }
            fixed_message.map(String::from).unwrap_or_else(|| self.to_string())
        };
        let body = json!({ "success": false, "error": { "code": code, "message": message } });
        (status, Json(body)).into_response()
    }
}
