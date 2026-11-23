use axum::{Extension, Json, extract::{Query, State}, http::StatusCode};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use super::{ApiResponse, extract::{ApiJson, ApiPath}};
use crate::{
    errors::AppResult,
    middleware::authentication::AuthUser,
    repositories::visitor_repository::VisitorIn,
    services::visitor_service as svc,
    state::SharedState,
};

type St = State<SharedState>;
type Me = Extension<AuthUser>;
type Ok<T> = AppResult<Json<ApiResponse<T>>>;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisitorBody {
    pub full_name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub visit_date: Option<NaiveDate>,
    pub how_heard: Option<String>,
    pub notes: Option<String>,
    pub status: Option<String>,
    pub assigned_to: Option<Uuid>,
}
impl From<VisitorBody> for VisitorIn {
    fn from(b: VisitorBody) -> Self {
        Self { full_name: b.full_name, phone: b.phone, email: b.email, visit_date: b.visit_date, how_heard: b.how_heard, notes: b.notes, status: b.status, assigned_to: b.assigned_to }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListQuery { pub status: Option<String> }
#[derive(Debug, Deserialize)]
pub struct AbsentQuery { pub weeks: Option<i32> }
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConvertBody { pub member_id: Option<Uuid> }
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogBody { pub kind: Option<String>, pub note: Option<String> }

pub async fn list(State(s): St, Extension(a): Me, Query(q): Query<ListQuery>) -> Ok<Vec<Value>> {
    Ok(Json(ApiResponse::ok(svc::list(&s, &a, q.status.as_deref()).await?)))
}
pub async fn get(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>) -> Ok<Value> {
    Ok(Json(ApiResponse::ok(svc::get(&s, &a, id).await?)))
}
pub async fn add(State(s): St, Extension(a): Me, ApiJson(b): ApiJson<VisitorBody>) -> AppResult<(StatusCode, Json<ApiResponse<Value>>)> {
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(svc::add(&s, &a, b.into()).await?))))
}
pub async fn update(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>, ApiJson(b): ApiJson<VisitorBody>) -> Ok<Value> {
    Ok(Json(ApiResponse::ok(svc::update(&s, &a, id, b.into()).await?)))
}
pub async fn remove(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>) -> AppResult<StatusCode> {
    svc::remove(&s, &a, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn convert(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>, ApiJson(b): ApiJson<ConvertBody>) -> Ok<Value> {
    Ok(Json(ApiResponse::ok(svc::convert(&s, &a, id, b.member_id).await?)))
}
pub async fn visitor_log(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>) -> Ok<Vec<Value>> {
    Ok(Json(ApiResponse::ok(svc::visitor_log(&s, &a, id).await?)))
}
pub async fn visitor_log_add(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>, ApiJson(b): ApiJson<LogBody>) -> AppResult<(StatusCode, Json<ApiResponse<Value>>)> {
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(svc::visitor_log_add(&s, &a, id, b.kind, b.note).await?))))
}
pub async fn member_log(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>) -> Ok<Vec<Value>> {
    Ok(Json(ApiResponse::ok(svc::member_log(&s, &a, id).await?)))
}
pub async fn member_log_add(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>, ApiJson(b): ApiJson<LogBody>) -> AppResult<(StatusCode, Json<ApiResponse<Value>>)> {
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(svc::member_log_add(&s, &a, id, b.kind, b.note).await?))))
}
pub async fn absent(State(s): St, Extension(a): Me, Query(q): Query<AbsentQuery>) -> Ok<Vec<Value>> {
    Ok(Json(ApiResponse::ok(svc::absent(&s, &a, q.weeks).await?)))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormLinkBody { pub enabled: Option<bool>, #[serde(default)] pub rotate: bool }

pub async fn form_link(State(s): St, Extension(a): Me) -> Ok<Value> {
    Ok(Json(ApiResponse::ok(svc::form_link(&s, &a).await?)))
}
pub async fn set_form_link(State(s): St, Extension(a): Me, ApiJson(b): ApiJson<FormLinkBody>) -> Ok<Value> {
    Ok(Json(ApiResponse::ok(svc::set_form_link(&s, &a, b.enabled, b.rotate).await?)))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicBody {
    pub full_name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub how_heard: Option<String>,
    pub notes: Option<String>,
    /// Hidden field: real people leave it empty.
    pub website: Option<String>,
}

pub async fn public_church(State(s): St, ApiPath(token): ApiPath<String>) -> Ok<Value> {
    Ok(Json(ApiResponse::ok(svc::public_church(&s, &token).await?)))
}
pub async fn public_submit(State(s): St, ApiPath(token): ApiPath<String>, ApiJson(b): ApiJson<PublicBody>) -> AppResult<(StatusCode, Json<ApiResponse<Value>>)> {
    let v = VisitorIn { full_name: b.full_name, phone: b.phone, email: b.email, visit_date: None, how_heard: b.how_heard, notes: b.notes, status: None, assigned_to: None };
    svc::public_submit(&s, &token, v, b.website).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(serde_json::json!({"received": true})))))
}
