use axum::http::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum FocError {
    #[error("The requested FOC was not found.")]
    NotFound,
}

impl FocError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "FOC_NOT_FOUND",
        }
    }
    pub fn status(&self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
        }
    }
}
