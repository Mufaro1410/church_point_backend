use axum::{Extension, Json, extract::State, http::StatusCode};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{ApiResponse, extract::{ApiJson, ApiPath, ApiQuery}};
use crate::{
    errors::AppResult,
    middleware::authentication::AuthUser,
    models::payment::{NewPayment, PaymentSettingsRequest, PaymentsQuery},
    services::payment_service,
    state::SharedState,
};

type Reply<T> = Json<ApiResponse<T>>;

pub async fn get_settings(State(s): State<SharedState>, Extension(a): Extension<AuthUser>) -> AppResult<Reply<Value>> {
    Ok(Json(ApiResponse::ok(payment_service::get_settings(&s, &a).await?)))
}

pub async fn put_settings(
    State(s): State<SharedState>, Extension(a): Extension<AuthUser>, ApiJson(b): ApiJson<PaymentSettingsRequest>,
) -> AppResult<Reply<Value>> {
    Ok(Json(ApiResponse::ok(payment_service::put_settings(&s, &a, b).await?)))
}

pub async fn list(
    State(s): State<SharedState>, Extension(a): Extension<AuthUser>, ApiQuery(q): ApiQuery<PaymentsQuery>,
) -> AppResult<Reply<Vec<Value>>> {
    Ok(Json(ApiResponse::ok(payment_service::list(&s, &a, q).await?)))
}

pub async fn start(
    State(s): State<SharedState>, Extension(a): Extension<AuthUser>, ApiJson(b): ApiJson<NewPayment>,
) -> AppResult<(StatusCode, Reply<Value>)> {
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(payment_service::start(&s, &a, b).await?))))
}

pub async fn get(
    State(s): State<SharedState>, Extension(a): Extension<AuthUser>, ApiPath(id): ApiPath<Uuid>,
) -> AppResult<Reply<Value>> {
    Ok(Json(ApiResponse::ok(payment_service::get(&s, &a, id).await?)))
}

/// Paynow posts form data here. No login: the signature is the proof.
pub async fn webhook(State(s): State<SharedState>, ApiPath(id): ApiPath<Uuid>, body: String) -> AppResult<Reply<Value>> {
    payment_service::webhook(&s, id, &body).await?;
    Ok(Json(ApiResponse::ok(json!({ "received": true }))))
}
