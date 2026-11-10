use axum::http::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum SermonError {
    #[error("The requested sermon was not found.")]
    NotFound,
}

impl SermonError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "SERMON_NOT_FOUND",
        }
    }
    pub fn status(&self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
        }
    }
}
