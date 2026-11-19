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
    pub foc_status: Option<String>,
    pub role: Option<String>,
    /// pending / approved / rejected / transfer_requested
    pub member_status: Option<String>,
    pub permissions: Vec<String>,
}

impl AuthUser {
    pub fn is_admin(&self) -> bool {
        matches!(self.role.as_deref(), Some("admin" | "super_admin"))
    }

    /// Admins can do everything in their church; everyone else needs the permission from a custom role.
    pub fn can(&self, permission: &str) -> bool {
        self.is_admin() || self.permissions.iter().any(|p| p == permission)
    }

    pub fn is_super_admin(&self) -> bool {
        self.role.as_deref() == Some("super_admin")
    }

    /// Most data belongs to a FOC; fail closed if the user has none.
    pub fn require_foc(&self) -> Result<Uuid, AuthError> {
        let foc = self.foc_id.ok_or(AuthError::NoFoc)?;
        if self.foc_status.as_deref() != Some("approved") && !self.is_super_admin() {
            return Err(AuthError::FocNotApproved);
        }
        Ok(foc)
    }
}

/// For the one endpoint a brand-new user can call before they have a member profile.
#[derive(Debug, Clone)]
pub struct TokenUser {
    pub user_id: Uuid,
}

pub async fn authenticate_token(
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
    req.extensions_mut().insert(TokenUser { user_id: claims.sub });
    Ok(next.run(req).await)
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

    let ttl = std::time::Duration::from_secs(state.settings.auth_cache_secs);
    let cached = if ttl.is_zero() {
        None
    } else {
        state.auth_cache.lock().ok().and_then(|c| {
            c.get(&claims.sub).filter(|(at, _)| at.elapsed() < ttl).map(|(_, p)| p.clone())
        })
    };
    let profile = match cached {
        Some(p) => p,
        None => {
            let p = user_repository::find_auth_profile(&state.db, claims.sub)
                .await?
                .ok_or(AuthError::NoProfile)?;
            if !ttl.is_zero() {
                if let Ok(mut c) = state.auth_cache.lock() {
                    if c.len() > 5000 {
                        c.clear();
                    }
                    c.insert(claims.sub, (std::time::Instant::now(), p.clone()));
                }
            }
            p
        }
    };

    req.extensions_mut().insert(AuthUser {
        user_id: claims.sub,
        member_id: profile.member_id,
        foc_id: profile.foc_id,
        church_id: profile.church_id,
        province_id: profile.province_id,
        foc_status: profile.foc_status,
        role: profile.role,
        member_status: profile.member_status,
        permissions: profile.permissions,
    });

    Ok(next.run(req).await)
}
