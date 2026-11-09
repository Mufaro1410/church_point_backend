use serde_json::Value;

use crate::{
    errors::{AppError, AppResult, AuthError, FocError},
    middleware::authentication::AuthUser,
    repositories::foc_repository,
    state::SharedState,
};

/// The caller's own FOC (replaces React `fetchFoc`).
pub async fn current(state: &SharedState, auth: &AuthUser) -> AppResult<Value> {
    let foc_id = auth.require_foc()?;
    foc_repository::find_by_id(&state.db, foc_id)
        .await?
        .ok_or_else(|| AppError::from(FocError::NotFound))
}

/// Provinces of the caller's church (replaces React `fetchProvinces`).
pub async fn provinces(state: &SharedState, auth: &AuthUser) -> AppResult<Vec<Value>> {
    let church_id = auth.church_id.ok_or(AuthError::NoFoc)?;
    Ok(foc_repository::list_provinces(&state.db, church_id).await?)
}
