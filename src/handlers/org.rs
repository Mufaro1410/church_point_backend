use axum::{Extension, Json, extract::State, http::StatusCode};
use serde_json::Value;
use uuid::Uuid;

use super::{ApiResponse, extract::{ApiJson, ApiPath}};
use crate::{
    errors::AppResult,
    middleware::authentication::AuthUser,
    models::org::{CellGroupRequest, MeetingRequest},
    services::org_service as svc,
    state::SharedState,
};

type Auth = Extension<AuthUser>;
type St = State<SharedState>;
type Out<T> = AppResult<Json<ApiResponse<T>>>;

fn ok<T: serde::Serialize>(v: T) -> Json<ApiResponse<T>> { Json(ApiResponse::ok(v)) }

pub async fn list_groups(State(s): St, Extension(a): Auth) -> Out<Vec<Value>> { Ok(ok(svc::list_groups(&s, &a).await?)) }
pub async fn get_group(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>) -> Out<Value> { Ok(ok(svc::get_group(&s, &a, id).await?)) }
pub async fn create_group(State(s): St, Extension(a): Auth, ApiJson(r): ApiJson<CellGroupRequest>) -> AppResult<(StatusCode, Json<ApiResponse<Value>>)> {
    Ok((StatusCode::CREATED, ok(svc::create_group(&s, &a, r).await?)))
}
pub async fn update_group(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>, ApiJson(r): ApiJson<CellGroupRequest>) -> Out<Value> {
    Ok(ok(svc::update_group(&s, &a, id, r).await?))
}
pub async fn delete_group(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>) -> AppResult<StatusCode> {
    svc::delete_group(&s, &a, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn list_all_meetings(State(s): St, Extension(a): Auth) -> Out<Vec<Value>> { Ok(ok(svc::list_all_meetings(&s, &a).await?)) }
pub async fn list_meetings(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>) -> Out<Vec<Value>> { Ok(ok(svc::list_meetings(&s, &a, id).await?)) }
pub async fn create_meeting(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>, ApiJson(r): ApiJson<MeetingRequest>) -> AppResult<(StatusCode, Json<ApiResponse<Value>>)> {
    Ok((StatusCode::CREATED, ok(svc::create_meeting(&s, &a, id, r).await?)))
}
pub async fn delete_meeting(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>) -> AppResult<StatusCode> {
    svc::delete_meeting(&s, &a, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn update_meeting(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>, ApiJson(r): ApiJson<MeetingRequest>) -> Out<Value> { Ok(ok(svc::update_meeting(&s, &a, id, r).await?)) }
pub async fn dashboard(State(s): St, Extension(a): Auth) -> Out<Value> { Ok(ok(svc::dashboard(&s, &a).await?)) }
