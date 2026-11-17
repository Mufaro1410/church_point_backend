use axum::{Extension, Json, extract::State};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use super::{ApiResponse, extract::{ApiJson, ApiPath, ApiQuery}};
use crate::{
    errors::AppResult, middleware::authentication::AuthUser, models::org::StatusRequest,
    services::platform_service, state::SharedState,
};

#[derive(Deserialize)]
pub struct FocFilter {
    pub status: Option<String>,
}

pub async fn list_focs(
    State(s): State<SharedState>,
    Extension(a): Extension<AuthUser>,
    ApiQuery(f): ApiQuery<FocFilter>,
) -> AppResult<Json<ApiResponse<Vec<Value>>>> {
    Ok(Json(ApiResponse::ok(platform_service::list_focs(&s, &a, f.status).await?)))
}

pub async fn set_status(
    State(s): State<SharedState>,
    Extension(a): Extension<AuthUser>,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(req): ApiJson<StatusRequest>,
) -> AppResult<Json<ApiResponse<Value>>> {
    Ok(Json(ApiResponse::ok(platform_service::set_status(&s, &a, id, req.status).await?)))
}
