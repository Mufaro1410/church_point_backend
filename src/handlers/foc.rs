use axum::{Extension, Json, extract::State};
use serde_json::Value;

use super::ApiResponse;
use crate::{
    errors::AppResult, middleware::authentication::AuthUser, services::foc_service,
    state::SharedState,
};

pub async fn current(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<Json<ApiResponse<Value>>> {
    Ok(Json(ApiResponse::ok(foc_service::current(&state, &auth).await?)))
}

pub async fn provinces(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<Json<ApiResponse<Vec<Value>>>> {
    Ok(Json(ApiResponse::ok(foc_service::provinces(&state, &auth).await?)))
}
