use axum::{Extension, Json};
use serde::Serialize;
use uuid::Uuid;

use super::ApiResponse;
use crate::middleware::authentication::AuthUser;

#[derive(Serialize)]
pub struct MeResponse {
    pub user_id: Uuid,
    pub member_id: Uuid,
    pub foc_id: Option<Uuid>,
    pub church_id: Option<Uuid>,
    pub province_id: Option<Uuid>,
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
        role: auth.role,
    }))
}
