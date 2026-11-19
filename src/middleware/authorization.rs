use axum::{extract::Request, middleware::Next, response::Response};

use super::authentication::AuthUser;
use crate::errors::{AppError, AuthError};

pub const ADMIN: &[&str] = &["admin", "super_admin"];
pub const SUPER_ADMIN: &[&str] = &["super_admin"];

/// Allows the request if the user is an admin, or a custom role gives them the permission for one of `areas`:
/// GET needs `area.view`, DELETE needs `area.delete`, anything else needs `area.manage`.
/// `["*"]` means any permission at all (for lookups every signed-in staff member needs).
/// Must run AFTER `authenticate`. Fails closed.
pub async fn require_perm(
    areas: &'static [&'static str],
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let user = req.extensions().get::<AuthUser>().ok_or(AuthError::MissingToken)?;
    let allowed = if user.is_admin() {
        true
    } else if areas == ["*"] {
        !user.permissions.is_empty()
    } else {
        let action = match *req.method() {
            axum::http::Method::GET | axum::http::Method::HEAD => "view",
            axum::http::Method::DELETE => "delete",
            _ => "manage",
        };
        areas.iter().any(|a| user.can(&format!("{a}.{action}")))
    };
    if allowed { Ok(next.run(req).await) } else { Err(AuthError::Forbidden.into()) }
}

/// Allows the request only if the user's role is in `allowed`. Fails closed.
/// Must run AFTER `authenticate`. (Only the "admin" role uses this API today; cell leaders are paused until custom roles and permissions exist.
/// We can evolve this into fine-grained permissions later.)
pub async fn require_roles(
    allowed: &'static [&'static str],
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let user = req.extensions().get::<AuthUser>().ok_or(AuthError::MissingToken)?;
    match user.role.as_deref() {
        Some(r) if allowed.contains(&r) => Ok(next.run(req).await),
        _ => Err(AuthError::Forbidden.into()),
    }
}
