use axum::http::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum PrayerError {
    #[error("The requested prayer was not found.")]
    NotFound,
}

impl PrayerError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "PRAYER_NOT_FOUND",
        }
    }
    pub fn status(&self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
        }
    }
}
