use axum::{
    extract::{Request, State},
    http::header::AUTHORIZATION,
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use crate::{
    errors::{AppError, AuthError},
    repositories::user_repository,
    state::SharedState,
    utils::jwt,
};

/// Identity attached to every authenticated request.
#[derive(Debug, Clone)]
pub struct AuthUser {
    /// Supabase auth.users.id
    pub user_id: Uuid,
    /// member_profiles.id
    pub member_id: Uuid,
    pub foc_id: Option<Uuid>,
    pub church_id: Option<Uuid>,
    pub province_id: Option<Uuid>,
    pub role: Option<String>,
}

impl AuthUser {
    /// Most data belongs to a FOC; fail closed if the user has none.
    pub fn require_foc(&self) -> Result<Uuid, AuthError> {
        self.foc_id.ok_or(AuthError::NoFoc)
    }
}

pub async fn authenticate(
    State(state): State<SharedState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AuthError::MissingToken)?;

    let claims = jwt::verify_supabase_token(token, &state.settings.supabase_jwt_secret)?;

    let profile = user_repository::find_auth_profile(&state.db, claims.sub)
        .await?
        .ok_or(AuthError::NoProfile)?;

    req.extensions_mut().insert(AuthUser {
        user_id: claims.sub,
        member_id: profile.member_id,
        foc_id: profile.foc_id,
        church_id: profile.church_id,
        province_id: profile.province_id,
        role: profile.role,
    });

    Ok(next.run(req).await)
}
