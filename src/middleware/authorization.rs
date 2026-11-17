use axum::{extract::Request, middleware::Next, response::Response};

use super::authentication::AuthUser;
use crate::errors::{AppError, AuthError};

pub const ADMIN: &[&str] = &["admin", "super_admin"];
pub const SUPER_ADMIN: &[&str] = &["super_admin"];

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
