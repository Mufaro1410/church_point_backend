use axum::http::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Authentication required.")]
    MissingToken,
    #[error("The authentication token is invalid or expired.")]
    InvalidToken,
    #[error("No member profile is linked to this account.")]
    NoProfile,
    #[error("Invalid email or password.")]
    InvalidCredentials,
    #[error("Please confirm your email first. Check your inbox for the confirmation link.")]
    EmailNotConfirmed,
    #[error("Your profile is not assigned to a FOC.")]
    NoFoc,
    #[error("Your FOC is not approved yet or has been suspended.")]
    FocNotApproved,
    #[error("You do not have permission to perform this action.")]
    Forbidden,
}

impl AuthError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::MissingToken => "MISSING_TOKEN",
            Self::InvalidToken => "INVALID_TOKEN",
            Self::NoProfile => "NO_PROFILE",
            Self::InvalidCredentials => "INVALID_CREDENTIALS",
            Self::EmailNotConfirmed => "EMAIL_NOT_CONFIRMED",
            Self::NoFoc => "NO_FOC",
            Self::FocNotApproved => "FOC_NOT_APPROVED",
            Self::Forbidden => "FORBIDDEN",
        }
    }
    pub fn status(&self) -> StatusCode {
        match self {
            Self::MissingToken | Self::InvalidToken | Self::InvalidCredentials => StatusCode::UNAUTHORIZED,
            Self::NoProfile | Self::EmailNotConfirmed | Self::NoFoc | Self::FocNotApproved | Self::Forbidden => StatusCode::FORBIDDEN,
        }
    }
}
