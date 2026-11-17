use axum::{Extension, Json, extract::State, http::StatusCode};
use serde_json::{Map, Value};
use uuid::Uuid;

use super::{ApiResponse, extract::{ApiJson, ApiPath, ApiQuery}};
use crate::{
    errors::AppResult,
    middleware::authentication::AuthUser,
    models::member::{FamilyRequest, MemberQuery, ReleaseRequest},
    services::member_service as svc,
    state::SharedState,
    utils::pagination::Paginated,
};

type Auth = Extension<AuthUser>;
type St = State<SharedState>;
type Out<T> = AppResult<Json<ApiResponse<T>>>;
fn ok<T: serde::Serialize>(v: T) -> Json<ApiResponse<T>> { Json(ApiResponse::ok(v)) }

pub async fn list(State(s): St, Extension(a): Auth, ApiQuery(q): ApiQuery<MemberQuery>) -> Out<Paginated<Value>> { Ok(ok(svc::list(&s, &a, q).await?)) }
pub async fn get(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>) -> Out<Value> { Ok(ok(svc::get(&s, &a, id).await?)) }
pub async fn create(State(s): St, Extension(a): Auth, ApiJson(b): ApiJson<Map<String, Value>>) -> AppResult<(StatusCode, Json<ApiResponse<Value>>)> {
    Ok((StatusCode::CREATED, ok(svc::create(&s, &a, b).await?)))
}
pub async fn update(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>, ApiJson(b): ApiJson<Map<String, Value>>) -> Out<Value> { Ok(ok(svc::update(&s, &a, id, b).await?)) }
pub async fn approve(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>) -> Out<Value> { Ok(ok(svc::approve(&s, &a, id).await?)) }
pub async fn set_pending(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>) -> Out<Value> { Ok(ok(svc::set_pending(&s, &a, id).await?)) }
pub async fn reject(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>) -> AppResult<StatusCode> {
    svc::reject(&s, &a, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn release(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>, ApiJson(r): ApiJson<ReleaseRequest>) -> AppResult<StatusCode> {
    svc::release(&s, &a, id, r).await?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn delete(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>) -> AppResult<StatusCode> {
    svc::delete(&s, &a, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn set_role(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>, ApiJson(r): ApiJson<crate::models::member::RoleRequest>) -> Out<Value> { Ok(ok(svc::set_role(&s, &a, id, &r.role).await?)) }
pub async fn family(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>) -> Out<Vec<Value>> { Ok(ok(svc::family(&s, &a, id).await?)) }
pub async fn add_family(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>, ApiJson(r): ApiJson<FamilyRequest>) -> AppResult<(StatusCode, Json<ApiResponse<Value>>)> {
    Ok((StatusCode::CREATED, ok(svc::add_family(&s, &a, id, r).await?)))
}
pub async fn remove_family(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>) -> AppResult<StatusCode> {
    svc::remove_family(&s, &a, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn contributions(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>) -> Out<Vec<Value>> { Ok(ok(svc::contributions(&s, &a, id).await?)) }
pub async fn attendance(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>) -> Out<Vec<Value>> { Ok(ok(svc::attendance(&s, &a, id).await?)) }
pub async fn activity(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>) -> Out<Vec<Value>> { Ok(ok(svc::activity(&s, &a, id).await?)) }
