use axum::{Extension, Json, extract::State, http::StatusCode};
use serde_json::Value;
use uuid::Uuid;

use super::{ApiResponse, extract::{ApiJson, ApiPath}};
use crate::{
    errors::AppResult,
    middleware::authentication::AuthUser,
    models::rbac::{AgeGroupRequest, AssignRolesRequest, RoleRequest},
    services::rbac_service as svc,
    state::SharedState,
};

type St = State<SharedState>;
type Me = Extension<AuthUser>;
type Out<T> = AppResult<Json<ApiResponse<T>>>;

pub async fn permissions() -> Out<Value> { Ok(Json(ApiResponse::ok(svc::catalog()))) }

pub async fn roles(State(s): St, Extension(a): Me) -> Out<Vec<Value>> { Ok(Json(ApiResponse::ok(svc::roles(&s, &a).await?))) }
pub async fn add_role(State(s): St, Extension(a): Me, ApiJson(b): ApiJson<RoleRequest>) -> AppResult<(StatusCode, Json<ApiResponse<Value>>)> {
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(svc::add_role(&s, &a, b).await?))))
}
pub async fn update_role(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>, ApiJson(b): ApiJson<RoleRequest>) -> Out<Value> {
    Ok(Json(ApiResponse::ok(svc::update_role(&s, &a, id, b).await?)))
}
pub async fn delete_role(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>) -> AppResult<StatusCode> {
    svc::delete_role(&s, &a, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn role_members(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>) -> Out<Vec<Value>> {
    Ok(Json(ApiResponse::ok(svc::role_members(&s, &a, id).await?)))
}
pub async fn member_roles(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>) -> Out<Vec<Value>> {
    Ok(Json(ApiResponse::ok(svc::member_roles(&s, &a, id).await?)))
}
pub async fn set_member_roles(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>, ApiJson(b): ApiJson<AssignRolesRequest>) -> Out<Vec<Value>> {
    Ok(Json(ApiResponse::ok(svc::set_member_roles(&s, &a, id, b).await?)))
}

pub async fn age_groups(State(s): St, Extension(a): Me) -> Out<Vec<Value>> { Ok(Json(ApiResponse::ok(svc::age_groups(&s, &a).await?))) }
pub async fn add_age_group(State(s): St, Extension(a): Me, ApiJson(b): ApiJson<AgeGroupRequest>) -> AppResult<(StatusCode, Json<ApiResponse<Value>>)> {
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(svc::add_age_group(&s, &a, b).await?))))
}
pub async fn update_age_group(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>, ApiJson(b): ApiJson<AgeGroupRequest>) -> Out<Value> {
    Ok(Json(ApiResponse::ok(svc::update_age_group(&s, &a, id, b).await?)))
}
pub async fn delete_age_group(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>) -> AppResult<StatusCode> {
    svc::delete_age_group(&s, &a, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
