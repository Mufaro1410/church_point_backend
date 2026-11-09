use axum::http::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum MemberError {
    #[error("The requested member was not found.")]
    NotFound,
}

impl MemberError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "MEMBER_NOT_FOUND",
        }
    }
    pub fn status(&self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
        }
    }
}
