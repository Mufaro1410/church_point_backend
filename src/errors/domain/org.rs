use axum::http::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum OrgError {
    #[error("The requested cell group was not found.")]
    CellGroupNotFound,
    #[error("The requested meeting was not found.")]
    MeetingNotFound,
    #[error("That member does not belong to your FOC.")]
    LeaderNotInFoc,
    #[error("{0} not found.")]
    NotFound(&'static str),
    #[error("{0} already exists.")]
    Duplicate(&'static str),
}

impl OrgError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::CellGroupNotFound => "CELL_GROUP_NOT_FOUND",
            Self::MeetingNotFound => "MEETING_NOT_FOUND",
            Self::LeaderNotInFoc => "LEADER_NOT_IN_FOC",
            Self::NotFound(_) => "NOT_FOUND",
            Self::Duplicate(_) => "DUPLICATE_VALUE",
        }
    }
    pub fn status(&self) -> StatusCode {
        match self {
            Self::CellGroupNotFound | Self::MeetingNotFound | Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Duplicate(_) => StatusCode::CONFLICT,
            Self::LeaderNotInFoc => StatusCode::UNPROCESSABLE_ENTITY,
        }
    }
}
