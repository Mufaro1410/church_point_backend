use axum::http::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum AnnouncementError {
    #[error("The requested announcement was not found.")]
    NotFound,
}

impl AnnouncementError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "ANNOUNCEMENT_NOT_FOUND",
        }
    }
    pub fn status(&self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
        }
    }
}
