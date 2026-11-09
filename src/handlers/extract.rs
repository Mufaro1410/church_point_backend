use axum::{
    Json,
    extract::{FromRequest, Request, rejection::JsonRejection},
};

use crate::errors::AppError;

/// Like `axum::Json`, but a bad/missing field becomes a normal 422 JSON error
/// ({ "success": false, "error": { "code": "VALIDATION_ERROR", ... } }).
pub struct ApiJson<T>(pub T);

impl<S, T> FromRequest<S> for ApiJson<T>
where
    S: Send + Sync,
    Json<T>: FromRequest<S, Rejection = JsonRejection>,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        Json::<T>::from_request(req, state)
            .await
            .map(|Json(v)| ApiJson(v))
            .map_err(|rej| AppError::Validation(rej.body_text()))
    }
}
