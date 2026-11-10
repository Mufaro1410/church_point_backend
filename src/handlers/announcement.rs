use axum::{Extension, Json, extract::State, http::StatusCode};
use serde_json::Value;
use uuid::Uuid;

use super::{ApiResponse, extract::{ApiJson, ApiPath, ApiQuery}};
use crate::{
    errors::AppResult,
    middleware::authentication::AuthUser,
    models::announcement::{AnnouncementsQuery, CreateAnnouncementRequest, UpdateAnnouncementRequest},
    services::announcement_service,
    state::SharedState,
};

type Reply<T> = Json<ApiResponse<T>>;

pub async fn list(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    ApiQuery(q): ApiQuery<AnnouncementsQuery>,
) -> AppResult<Reply<Vec<Value>>> {
    Ok(Json(ApiResponse::ok(announcement_service::list(&state, &auth, q).await?)))
}

pub async fn get(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    ApiPath(id): ApiPath<Uuid>,
) -> AppResult<Reply<Value>> {
    Ok(Json(ApiResponse::ok(announcement_service::get(&state, &auth, id).await?)))
}

pub async fn create(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    ApiJson(body): ApiJson<CreateAnnouncementRequest>,
) -> AppResult<(StatusCode, Reply<Value>)> {
    let v = announcement_service::create(&state, &auth, body).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(v))))
}

pub async fn update(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(body): ApiJson<UpdateAnnouncementRequest>,
) -> AppResult<Reply<Value>> {
    Ok(Json(ApiResponse::ok(announcement_service::update(&state, &auth, id, body).await?)))
}

pub async fn delete(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    ApiPath(id): ApiPath<Uuid>,
) -> AppResult<StatusCode> {
    announcement_service::delete(&state, &auth, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
