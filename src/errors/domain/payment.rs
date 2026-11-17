use axum::http::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum PaymentError {
    #[error("The requested payment was not found.")]
    NotFound,
    #[error("Online payments are not set up for your church yet.")]
    NotSetUp,
    #[error("Check the phone number: EcoCash uses 077/078 and OneMoney uses 071 (10 digits).")]
    BadPhone,
    #[error("Payment settings are incomplete.")]
    BadSettings,
}

impl PaymentError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "PAYMENT_NOT_FOUND",
            Self::NotSetUp => "PAYMENTS_NOT_SET_UP",
            Self::BadPhone => "INVALID_PHONE",
            Self::BadSettings => "INVALID_PAYMENT_SETTINGS",
        }
    }
    pub fn status(&self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::NotSetUp => StatusCode::CONFLICT,
            Self::BadPhone | Self::BadSettings => StatusCode::UNPROCESSABLE_ENTITY,
        }
    }
}
