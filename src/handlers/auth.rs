use axum::{Extension, Json, extract::State, http::{HeaderMap, StatusCode, header::AUTHORIZATION}};
use serde::Serialize;
use uuid::Uuid;

use super::ApiResponse;
use crate::{
    errors::AppResult,
    handlers::extract::ApiJson,
    middleware::authentication::{AuthUser, TokenUser},
    models::auth::{ForgotRequest, LoginRequest, RefreshRequest, RegisterRequest},
    models::catalog::RegisterFoc,
    services::{auth_service, catalog_service},
    state::SharedState,
};

#[derive(Serialize)]
pub struct MeResponse {
    pub user_id: Uuid,
    pub member_id: Uuid,
    pub foc_id: Option<Uuid>,
    pub church_id: Option<Uuid>,
    pub province_id: Option<Uuid>,
    pub foc_status: Option<String>,
    pub role: Option<String>,
}

/// Replaces the React `fetchProfile` identity lookup. Everything here was already
/// resolved by the `authenticate` middleware.
pub async fn me(Extension(auth): Extension<AuthUser>) -> Json<ApiResponse<MeResponse>> {
    Json(ApiResponse::ok(MeResponse {
        user_id: auth.user_id,
        member_id: auth.member_id,
        foc_id: auth.foc_id,
        church_id: auth.church_id,
        province_id: auth.province_id,
        foc_status: auth.foc_status,
        role: auth.role,
    }))
}

/// A signed-up user registers their FOC. It starts `pending` until the platform owner approves it.
pub async fn register_foc(
    State(s): State<SharedState>,
    Extension(t): Extension<TokenUser>,
    ApiJson(req): ApiJson<RegisterFoc>,
) -> AppResult<(StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(catalog_service::register_foc(&s, t.user_id, req).await?))))
}

type Reply = AppResult<Json<ApiResponse<serde_json::Value>>>;

pub async fn login(State(s): State<SharedState>, ApiJson(r): ApiJson<LoginRequest>) -> Reply {
    Ok(Json(ApiResponse::ok(auth_service::login(&s, r).await?)))
}

pub async fn refresh(State(s): State<SharedState>, ApiJson(r): ApiJson<RefreshRequest>) -> Reply {
    Ok(Json(ApiResponse::ok(auth_service::refresh(&s, r).await?)))
}

pub async fn register(State(s): State<SharedState>, ApiJson(r): ApiJson<RegisterRequest>) -> AppResult<(StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(auth_service::register(&s, r).await?))))
}

pub async fn forgot_password(State(s): State<SharedState>, ApiJson(r): ApiJson<ForgotRequest>) -> AppResult<StatusCode> {
    auth_service::forgot_password(&s, r).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangePassword {
    pub password: String,
}

pub async fn change_password(State(s): State<SharedState>, headers: HeaderMap, ApiJson(b): ApiJson<ChangePassword>) -> AppResult<StatusCode> {
    let token = headers.get(AUTHORIZATION).and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")).unwrap_or_default();
    auth_service::change_password(&s, token, &b.password).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn logout(State(s): State<SharedState>, headers: HeaderMap) -> AppResult<StatusCode> {
    let token = headers.get(AUTHORIZATION).and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")).unwrap_or_default();
    auth_service::logout(&s, token).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Full profile of the signed-in person (what the apps show in the header and settings).
pub async fn profile(State(s): State<SharedState>, Extension(a): Extension<AuthUser>) -> Reply {
    let p = crate::repositories::user_repository::own_profile(&s.db, a.member_id).await?;
    Ok(Json(ApiResponse::ok(p.unwrap_or(serde_json::Value::Null))))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResetPassword {
    pub access_token: String,
    pub password: String,
}

pub async fn reset_password(State(s): State<SharedState>, ApiJson(b): ApiJson<ResetPassword>) -> AppResult<StatusCode> {
    auth_service::reset_password(&s, &b.access_token, &b.password).await?;
    Ok(StatusCode::NO_CONTENT)
}
