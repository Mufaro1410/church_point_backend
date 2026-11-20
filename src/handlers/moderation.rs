use axum::{Extension, Json, extract::State, http::StatusCode};
use serde_json::Value;
use uuid::Uuid;

use super::{ApiResponse, extract::{ApiJson, ApiPath}};
use crate::{
    errors::AppResult,
    middleware::authentication::AuthUser,
    models::moderation::{ActionRequest, BlockRequest, ReportRequest},
    services::moderation_service as svc,
    state::SharedState,
};

type St = State<SharedState>;
type Me = Extension<AuthUser>;
type Out<T> = AppResult<Json<ApiResponse<T>>>;

fn ok<T: serde::Serialize>(v: T) -> Json<ApiResponse<T>> { Json(ApiResponse::ok(v)) }

pub async fn report(State(s): St, Extension(a): Me, ApiJson(b): ApiJson<ReportRequest>) -> AppResult<(StatusCode, Json<ApiResponse<Value>>)> {
    Ok((StatusCode::CREATED, ok(svc::report(&s, &a, b).await?)))
}
pub async fn block(State(s): St, Extension(a): Me, ApiJson(b): ApiJson<BlockRequest>) -> AppResult<(StatusCode, Json<ApiResponse<Value>>)> {
    Ok((StatusCode::CREATED, ok(svc::block(&s, &a, b).await?)))
}
pub async fn blocks(State(s): St, Extension(a): Me) -> Out<Vec<Value>> { Ok(ok(svc::blocks(&s, &a).await?)) }
pub async fn unblock(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>) -> AppResult<StatusCode> {
    svc::unblock(&s, &a, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn accept_terms(State(s): St, Extension(a): Me) -> Out<Value> { Ok(ok(svc::accept_terms(&s, &a).await?)) }

pub async fn open_reports(State(s): St, Extension(a): Me) -> Out<Vec<Value>> { Ok(ok(svc::open_reports(&s, &a).await?)) }
pub async fn open_count(State(s): St, Extension(a): Me) -> Out<Value> { Ok(ok(svc::open_count(&s, &a).await?)) }
pub async fn act(State(s): St, Extension(a): Me, ApiPath((kind, id)): ApiPath<(String, Uuid)>, ApiJson(b): ApiJson<ActionRequest>) -> AppResult<StatusCode> {
    svc::act(&s, &a, &kind, id, b).await?;
    Ok(StatusCode::NO_CONTENT)
}
