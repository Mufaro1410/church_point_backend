use axum::{Extension, Json, extract::State, http::StatusCode};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use super::{ApiResponse, extract::{ApiJson, ApiPath}};
use crate::{errors::AppResult, middleware::authentication::AuthUser, services::capacity_service as svc, state::SharedState};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameBody {
    pub name: String,
}

type St = State<SharedState>;
type Me = Extension<AuthUser>;

pub async fn list(State(s): St, Extension(a): Me) -> AppResult<Json<ApiResponse<Vec<Value>>>> {
    Ok(Json(ApiResponse::ok(svc::list(&s, &a).await?)))
}
pub async fn add(State(s): St, Extension(a): Me, ApiJson(b): ApiJson<NameBody>) -> AppResult<(StatusCode, Json<ApiResponse<Value>>)> {
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(svc::add(&s, &a, &b.name).await?))))
}
pub async fn rename(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>, ApiJson(b): ApiJson<NameBody>) -> AppResult<Json<ApiResponse<Value>>> {
    Ok(Json(ApiResponse::ok(svc::rename(&s, &a, id, &b.name).await?)))
}
pub async fn remove(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>) -> AppResult<StatusCode> {
    svc::remove(&s, &a, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
