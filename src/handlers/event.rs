use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde_json::Value;
use uuid::Uuid;

use super::{ApiResponse, extract::ApiJson};
use crate::{
    errors::AppResult,
    middleware::authentication::AuthUser,
    models::event::{
        AttendanceSummary, CreateEventRequest, EventsQuery, MarkAttendanceRequest, RsvpStats,
        UpdateEventRequest,
    },
    services::event_service,
    state::SharedState,
};

pub async fn list(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    Query(q): Query<EventsQuery>,
) -> AppResult<Json<ApiResponse<Vec<Value>>>> {
    Ok(Json(ApiResponse::ok(event_service::list(&state, &auth, q).await?)))
}

pub async fn get(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ApiResponse<Value>>> {
    Ok(Json(ApiResponse::ok(event_service::get(&state, &auth, id).await?)))
}

pub async fn create(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    ApiJson(body): ApiJson<CreateEventRequest>,
) -> AppResult<(StatusCode, Json<ApiResponse<Value>>)> {
    let event = event_service::create(&state, &auth, body).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(event))))
}

pub async fn update(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    ApiJson(body): ApiJson<UpdateEventRequest>,
) -> AppResult<Json<ApiResponse<Value>>> {
    Ok(Json(ApiResponse::ok(event_service::update(&state, &auth, id, body).await?)))
}

pub async fn delete(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    event_service::delete(&state, &auth, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn rsvp_stats(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ApiResponse<RsvpStats>>> {
    Ok(Json(ApiResponse::ok(event_service::rsvp_stats(&state, &auth, id).await?)))
}

pub async fn attendance(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ApiResponse<AttendanceSummary>>> {
    Ok(Json(ApiResponse::ok(event_service::attendance(&state, &auth, id).await?)))
}

pub async fn mark_attendance(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    ApiJson(req): ApiJson<MarkAttendanceRequest>,
) -> AppResult<Json<ApiResponse<Value>>> {
    Ok(Json(ApiResponse::ok(event_service::mark_attendance(&state, &auth, id, req).await?)))
}

pub async fn regenerate_qr(
    State(state): State<SharedState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ApiResponse<Value>>> {
    Ok(Json(ApiResponse::ok(event_service::regenerate_qr(&state, &auth, id).await?)))
}
