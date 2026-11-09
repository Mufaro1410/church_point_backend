use axum::http::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum EventError {
    #[error("The requested event was not found.")]
    NotFound,
    #[error("This event has linked records (income, serving, etc.) and cannot be deleted.")]
    InUse,
}

impl EventError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "EVENT_NOT_FOUND",
            Self::InUse => "EVENT_IN_USE",
        }
    }
    pub fn status(&self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::InUse => StatusCode::CONFLICT,
        }
    }
}
