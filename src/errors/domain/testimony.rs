use axum::http::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum TestimonyError {
    #[error("The requested testimony was not found.")]
    NotFound,
}

impl TestimonyError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "TESTIMONY_NOT_FOUND",
        }
    }
    pub fn status(&self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
        }
    }
}
