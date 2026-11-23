use axum::{Extension, Json, extract::{Query, State}};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::Value;

use super::ApiResponse;
use crate::{errors::AppResult, middleware::authentication::AuthUser, services::report_service as svc, state::SharedState};

#[derive(Debug, Deserialize)]
pub struct Period { pub from: Option<NaiveDate>, pub to: Option<NaiveDate> }

pub async fn overview(State(s): State<SharedState>, Extension(a): Extension<AuthUser>, Query(q): Query<Period>) -> AppResult<Json<ApiResponse<Value>>> {
    Ok(Json(ApiResponse::ok(svc::overview(&s, &a, q.from, q.to).await?)))
}
pub async fn attendance(State(s): State<SharedState>, Extension(a): Extension<AuthUser>, Query(q): Query<Period>) -> AppResult<Json<ApiResponse<Value>>> {
    Ok(Json(ApiResponse::ok(svc::attendance(&s, &a, q.from, q.to).await?)))
}
pub async fn giving(State(s): State<SharedState>, Extension(a): Extension<AuthUser>, Query(q): Query<Period>) -> AppResult<Json<ApiResponse<Value>>> {
    Ok(Json(ApiResponse::ok(svc::giving(&s, &a, q.from, q.to).await?)))
}
