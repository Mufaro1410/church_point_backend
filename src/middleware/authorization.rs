use axum::{extract::Request, middleware::Next, response::Response};

use super::authentication::AuthUser;
use crate::errors::{AppError, AuthError};

/// Allows the request only if the user's role is in `allowed`. Fails closed.
/// Must run AFTER `authenticate`. (Roles today: "admin", "cell_leader".
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
