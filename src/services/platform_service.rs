use serde_json::Value;
use uuid::Uuid;

use crate::{
    errors::{AppError, AppResult, FocError},
    middleware::authentication::AuthUser,
    repositories::platform_repository,
    state::SharedState,
};

const STATUSES: [&str; 3] = ["pending", "approved", "suspended"];

fn guard(auth: &AuthUser) -> AppResult<()> {
    if auth.is_super_admin() { Ok(()) } else { Err(crate::errors::AuthError::Forbidden.into()) }
}

pub async fn list_focs(state: &SharedState, auth: &AuthUser, status: Option<String>) -> AppResult<Vec<Value>> {
    guard(auth)?;
    if let Some(s) = status.as_deref() {
        if !STATUSES.contains(&s) {
            return Err(AppError::Validation("status must be pending, approved or suspended.".into()));
        }
    }
    Ok(platform_repository::list_focs(&state.db, status.as_deref()).await?)
}

pub async fn set_status(state: &SharedState, auth: &AuthUser, id: Uuid, status: String) -> AppResult<Value> {
    guard(auth)?;
    if !STATUSES.contains(&status.as_str()) {
        return Err(AppError::Validation("status must be pending, approved or suspended.".into()));
    }
    let r = platform_repository::set_status(&state.db, id, &status).await?;
    state.clear_auth_cache();
    r.ok_or_else(|| FocError::NotFound.into())
}
