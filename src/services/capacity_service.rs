//! Serving capacities: each church's own list of jobs members can serve in. Managed by that church's admins.

use serde_json::Value;
use uuid::Uuid;

use crate::{errors::{AppError, AppResult}, middleware::authentication::AuthUser, repositories::capacity_repository as repo, state::SharedState};

fn clean(name: &str) -> AppResult<String> {
    let n = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if n.is_empty() || n.chars().count() > 60 {
        return Err(AppError::Validation("name is required (max 60 characters).".into()));
    }
    Ok(n)
}

fn duplicate(e: sqlx::Error) -> AppError {
    if e.as_database_error().is_some_and(|d| d.is_unique_violation()) {
        AppError::Validation("That serving capacity already exists.".into())
    } else {
        e.into()
    }
}

pub async fn list(state: &SharedState, auth: &AuthUser) -> AppResult<Vec<Value>> {
    Ok(repo::list(&state.db, auth.require_foc()?).await?)
}

pub async fn add(state: &SharedState, auth: &AuthUser, name: &str) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    repo::add(&state.db, foc, &clean(name)?).await.map_err(duplicate)
}

pub async fn rename(state: &SharedState, auth: &AuthUser, id: Uuid, name: &str) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    repo::rename(&state.db, foc, id, &clean(name)?).await.map_err(duplicate)?
        .ok_or_else(|| AppError::Validation("Serving capacity not found.".into()))
}

pub async fn remove(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<()> {
    let foc = auth.require_foc()?;
    if repo::remove(&state.db, foc, id).await? { Ok(()) } else { Err(AppError::Validation("Serving capacity not found.".into())) }
}
