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

use axum::{
    extract::{
        FromRequestParts, Path, Query,
        rejection::{PathRejection, QueryRejection},
    },
    http::request::Parts,
};
use serde::de::DeserializeOwned;

/// `axum::extract::Query` with our JSON error envelope.
pub struct ApiQuery<T>(pub T);

impl<S, T> FromRequestParts<S> for ApiQuery<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
    Query<T>: FromRequestParts<S, Rejection = QueryRejection>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Query::<T>::from_request_parts(parts, state)
            .await
            .map(|Query(v)| ApiQuery(v))
            .map_err(|rej| AppError::Validation(rej.body_text()))
    }
}

/// `axum::extract::Path` with our JSON error envelope (e.g. a malformed UUID).
pub struct ApiPath<T>(pub T);

impl<S, T> FromRequestParts<S> for ApiPath<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Send,
    Path<T>: FromRequestParts<S, Rejection = PathRejection>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Path::<T>::from_request_parts(parts, state)
            .await
            .map(|Path(v)| ApiPath(v))
            .map_err(|rej| AppError::Validation(rej.body_text()))
    }
}
