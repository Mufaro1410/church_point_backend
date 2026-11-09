use uuid::Uuid;

use crate::{
    errors::{AppError, AppResult, MemberError},
    middleware::authentication::AuthUser,
    models::member::{Member, UpdateMemberRequest},
    repositories::member_repository,
    state::SharedState,
    utils::pagination::{Paginated, PaginationParams},
};

pub async fn list(
    state: &SharedState,
    auth: &AuthUser,
    params: PaginationParams,
) -> AppResult<Paginated<Member>> {
    let foc_id = auth.require_foc()?;
    let items =
        member_repository::list_by_foc(&state.db, foc_id, params.limit(), params.offset()).await?;
    let total = member_repository::count_by_foc(&state.db, foc_id).await?;
    Ok(Paginated::new(items, total, &params))
}

pub async fn get(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Member> {
    let foc_id = auth.require_foc()?;
    member_repository::find_in_foc(&state.db, id, foc_id)
        .await?
        .ok_or_else(|| MemberError::NotFound.into())
}

/// Business rule: an admin must not remove or reject their own profile.
fn forbid_self(auth: &AuthUser, id: Uuid) -> AppResult<()> {
    if auth.member_id == id {
        return Err(AppError::Validation("You cannot do this to your own profile.".into()));
    }
    Ok(())
}

fn clean(field: &str, value: Option<String>) -> AppResult<Option<String>> {
    match value.map(|v| v.trim().to_string()) {
        Some(v) if v.is_empty() => Err(AppError::Validation(format!("{field} cannot be empty."))),
        other => Ok(other),
    }
}

pub async fn update(
    state: &SharedState,
    auth: &AuthUser,
    id: Uuid,
    req: UpdateMemberRequest,
) -> AppResult<Member> {
    let foc_id = auth.require_foc()?;
    let first = clean("first_name", req.first_name)?;
    let last = clean("last_name", req.last_name)?;
    member_repository::update_basic(&state.db, id, foc_id, first.as_deref(), last.as_deref())
        .await?
        .ok_or_else(|| MemberError::NotFound.into())
}

pub async fn approve(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Member> {
    let foc_id = auth.require_foc()?;
    member_repository::approve(&state.db, id, foc_id)
        .await?
        .ok_or_else(|| MemberError::NotFound.into())
}

pub async fn reject(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<()> {
    let foc_id = auth.require_foc()?;
    forbid_self(auth, id)?;
    match member_repository::reject_and_release(&state.db, id, foc_id).await? {
        0 => Err(MemberError::NotFound.into()),
        _ => Ok(()),
    }
}

pub async fn delete(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<()> {
    let foc_id = auth.require_foc()?;
    forbid_self(auth, id)?;
    match member_repository::soft_delete(&state.db, id, foc_id).await? {
        0 => Err(MemberError::NotFound.into()),
        _ => Ok(()),
    }
}
