use axum::{Extension, Json, extract::State, http::StatusCode};
use serde_json::{Map, Value};
use uuid::Uuid;

use super::{ApiResponse, extract::{ApiJson, ApiPath}};
use crate::{
    errors::AppResult,
    middleware::authentication::AuthUser,
    models::catalog::*,
    services::catalog_service as svc,
    state::SharedState,
};

type Auth = Extension<AuthUser>;
type St = State<SharedState>;
type Out<T> = AppResult<Json<ApiResponse<T>>>;
type Created = AppResult<(StatusCode, Json<ApiResponse<Value>>)>;
type Body = ApiJson<Map<String, Value>>;
fn ok<T: serde::Serialize>(v: T) -> Json<ApiResponse<T>> { Json(ApiResponse::ok(v)) }
fn created(v: Value) -> (StatusCode, Json<ApiResponse<Value>>) { (StatusCode::CREATED, ok(v)) }
const NO: StatusCode = StatusCode::NO_CONTENT;

pub async fn ministries(State(s): St, Extension(a): Auth) -> Out<Vec<Value>> { Ok(ok(svc::ministries(&s, &a).await?)) }
pub async fn add_ministry(State(s): St, Extension(a): Auth, ApiJson(r): ApiJson<NameRequest>) -> Created { Ok(created(svc::add_ministry(&s, &a, r).await?)) }
pub async fn select_ministry(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>, ApiJson(r): ApiJson<SelectedRequest>) -> AppResult<StatusCode> {
    svc::select_ministry(&s, &a, id, r.selected).await?;
    Ok(NO)
}
pub async fn set_leader(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>, ApiJson(r): ApiJson<LeaderRequest>) -> Out<Value> { Ok(ok(svc::set_leader(&s, &a, id, r.leader_id).await?)) }
pub async fn rename_ministry(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>, ApiJson(r): ApiJson<NameRequest>) -> Out<Value> { Ok(ok(svc::rename_ministry(&s, &a, id, r).await?)) }
pub async fn delete_ministry(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>) -> AppResult<StatusCode> { svc::delete_ministry(&s, &a, id).await?; Ok(NO) }

pub async fn courses(State(s): St, Extension(a): Auth) -> Out<Vec<Value>> { Ok(ok(svc::courses(&s, &a).await?)) }
pub async fn add_course(State(s): St, Extension(a): Auth, ApiJson(r): ApiJson<NameRequest>) -> Created { Ok(created(svc::add_course(&s, &a, r).await?)) }
pub async fn rename_course(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>, ApiJson(r): ApiJson<NameRequest>) -> Out<Value> { Ok(ok(svc::rename_course(&s, &a, id, r).await?)) }
pub async fn delete_course(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>) -> AppResult<StatusCode> { svc::delete_course(&s, &a, id).await?; Ok(NO) }

pub async fn list_serving(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>) -> Out<Vec<Value>> { Ok(ok(svc::list_serving(&s, &a, id).await?)) }
pub async fn all_serving(State(s): St, Extension(a): Auth) -> Out<Vec<Value>> { Ok(ok(svc::all_serving(&s, &a).await?)) }
pub async fn add_serving(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>, ApiJson(r): ApiJson<ServingRequest>) -> Created { Ok(created(svc::add_serving(&s, &a, id, r).await?)) }
pub async fn remove_serving(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>) -> AppResult<StatusCode> { svc::remove_serving(&s, &a, id).await?; Ok(NO) }

pub async fn list_services(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>) -> Out<Vec<Value>> { Ok(ok(svc::list_services(&s, &a, id).await?)) }
pub async fn add_service(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>, ApiJson(r): ApiJson<ServiceRequest>) -> Created { Ok(created(svc::add_service(&s, &a, id, r).await?)) }
pub async fn remove_service(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>) -> AppResult<StatusCode> { svc::remove_service(&s, &a, id).await?; Ok(NO) }
pub async fn logos(State(s): St, Extension(a): Auth) -> Out<Vec<String>> { Ok(ok(svc::logos(&s, &a).await?)) }

pub async fn social_list(State(s): St, Extension(a): Auth) -> Out<Vec<Value>> { Ok(ok(svc::social_list(&s, &a).await?)) }
pub async fn social_create(State(s): St, Extension(a): Auth, ApiJson(b): Body) -> Created { Ok(created(svc::social_create(&s, &a, b).await?)) }
pub async fn social_update(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>, ApiJson(b): Body) -> Out<Value> { Ok(ok(svc::social_update(&s, &a, id, b).await?)) }
pub async fn social_delete(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>) -> AppResult<StatusCode> { svc::social_delete(&s, &a, id).await?; Ok(NO) }

pub async fn plans(State(s): St, Extension(a): Auth) -> Out<Vec<Value>> { Ok(ok(svc::plans(&s, &a).await?)) }
pub async fn plan_create(State(s): St, Extension(a): Auth, ApiJson(b): Body) -> Created { Ok(created(svc::plan_create(&s, &a, b).await?)) }
pub async fn plan_update(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>, ApiJson(b): Body) -> Out<Value> { Ok(ok(svc::plan_update(&s, &a, id, b).await?)) }
pub async fn plan_delete(State(s): St, Extension(a): Auth, ApiPath(id): ApiPath<Uuid>) -> AppResult<StatusCode> { svc::plan_delete(&s, &a, id).await?; Ok(NO) }

pub async fn church(State(s): St, Extension(a): Auth) -> Out<Value> { Ok(ok(svc::church(&s, &a).await?)) }
pub async fn focs(State(s): St, Extension(a): Auth) -> Out<Vec<Value>> { Ok(ok(svc::focs(&s, &a).await?)) }
pub async fn update_foc(State(s): St, Extension(a): Auth, ApiJson(b): Body) -> Out<Value> { Ok(ok(svc::update_foc(&s, &a, b).await?)) }
