use axum::{Extension, Json, extract::State, http::StatusCode};
use serde_json::Value;
use uuid::Uuid;

use super::{ApiResponse, extract::{ApiJson, ApiPath, ApiQuery}};
use crate::{
    errors::AppResult,
    middleware::authentication::AuthUser,
    models::sermon::{CreateSermonRequest, SermonsQuery, UpdateSermonRequest},
    services::sermon_service,
    state::SharedState,
};

type Reply<T> = Json<ApiResponse<T>>;

pub async fn list(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    ApiQuery(q): ApiQuery<SermonsQuery>,
) -> AppResult<Reply<Vec<Value>>> {
    Ok(Json(ApiResponse::ok(sermon_service::list(&state, &auth, q).await?)))
}

pub async fn get(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    ApiPath(id): ApiPath<Uuid>,
) -> AppResult<Reply<Value>> {
    Ok(Json(ApiResponse::ok(sermon_service::get(&state, &auth, id).await?)))
}

pub async fn create(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    ApiJson(body): ApiJson<CreateSermonRequest>,
) -> AppResult<(StatusCode, Reply<Value>)> {
    let v = sermon_service::create(&state, &auth, body).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(v))))
}

pub async fn update(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(body): ApiJson<UpdateSermonRequest>,
) -> AppResult<Reply<Value>> {
    Ok(Json(ApiResponse::ok(sermon_service::update(&state, &auth, id, body).await?)))
}

pub async fn delete(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    ApiPath(id): ApiPath<Uuid>,
) -> AppResult<StatusCode> {
    sermon_service::delete(&state, &auth, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn audio_upload_url(State(s): State<SharedState>, Extension(a): Extension<AuthUser>, ApiPath(id): ApiPath<Uuid>) -> AppResult<Reply<Value>> {
    Ok(Json(ApiResponse::ok(sermon_service::audio_upload_url(&s, &a, id).await?)))
}
pub async fn audio_confirm(State(s): State<SharedState>, Extension(a): Extension<AuthUser>, ApiPath(id): ApiPath<Uuid>) -> AppResult<Reply<Value>> {
    Ok(Json(ApiResponse::ok(sermon_service::audio_confirm(&s, &a, id).await?)))
}
pub async fn audio_delete(State(s): State<SharedState>, Extension(a): Extension<AuthUser>, ApiPath(id): ApiPath<Uuid>) -> AppResult<Reply<Value>> {
    Ok(Json(ApiResponse::ok(sermon_service::audio_delete(&s, &a, id).await?)))
}

/// Raw audio body in; compressed, stored and linked sermon out.
pub async fn audio_file(State(s): State<SharedState>, Extension(a): Extension<AuthUser>, ApiPath(id): ApiPath<Uuid>, body: axum::body::Body) -> AppResult<Reply<Value>> {
    Ok(Json(ApiResponse::ok(sermon_service::audio_from_body(&s, &a, id, body).await?)))
}
