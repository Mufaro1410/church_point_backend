use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use uuid::Uuid;

use super::ApiResponse;
use crate::{
    errors::AppResult,
    middleware::authentication::AuthUser,
    models::member::Member,
    services::member_service,
    state::SharedState,
    utils::pagination::{Paginated, PaginationParams},
};

pub async fn list(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    Query(params): Query<PaginationParams>,
) -> AppResult<Json<ApiResponse<Paginated<Member>>>> {
    Ok(Json(ApiResponse::ok(member_service::list(&state, &auth, params).await?)))
}

pub async fn get(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ApiResponse<Member>>> {
    Ok(Json(ApiResponse::ok(member_service::get(&state, &auth, id).await?)))
}

pub async fn update(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    super::extract::ApiJson(req): super::extract::ApiJson<crate::models::member::UpdateMemberRequest>,
) -> AppResult<Json<ApiResponse<Member>>> {
    Ok(Json(ApiResponse::ok(member_service::update(&state, &auth, id, req).await?)))
}

pub async fn approve(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ApiResponse<Member>>> {
    Ok(Json(ApiResponse::ok(member_service::approve(&state, &auth, id).await?)))
}

pub async fn reject(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<axum::http::StatusCode> {
    member_service::reject(&state, &auth, id).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn delete(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<axum::http::StatusCode> {
    member_service::delete(&state, &auth, id).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}
