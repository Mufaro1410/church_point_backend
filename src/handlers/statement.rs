use axum::{Extension, Json, extract::{Query, State}};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use super::{ApiResponse, extract::ApiPath};
use crate::{errors::AppResult, middleware::authentication::AuthUser, services::statement_service as svc, state::SharedState};

#[derive(Debug, Deserialize)]
pub struct Period { pub from: Option<NaiveDate>, pub to: Option<NaiveDate> }

pub async fn summary(State(s): State<SharedState>, Extension(a): Extension<AuthUser>, Query(q): Query<Period>) -> AppResult<Json<ApiResponse<Value>>> {
    Ok(Json(ApiResponse::ok(svc::summary(&s, &a, q.from, q.to).await?)))
}
pub async fn statement(State(s): State<SharedState>, Extension(a): Extension<AuthUser>, ApiPath(id): ApiPath<Uuid>, Query(q): Query<Period>) -> AppResult<Json<ApiResponse<Value>>> {
    Ok(Json(ApiResponse::ok(svc::statement(&s, &a, id, q.from, q.to).await?)))
}
