//! Custom roles and age groups. Both belong to one church and are managed by that church's admins.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    errors::{AppError, AppResult, AuthError, MemberError},
    middleware::authentication::AuthUser,
    models::rbac::{AREAS, AgeGroupRequest, AssignRolesRequest, RoleRequest, normalise},
    repositories::{member_repository, rbac_repository as repo},
    state::SharedState,
};

fn clean(name: &str, what: &str) -> AppResult<String> {
    let n = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if n.is_empty() || n.chars().count() > 60 {
        return Err(AppError::Validation(format!("{what} name is required (max 60 characters).")));
    }
    Ok(n)
}

fn duplicate(what: &'static str) -> impl Fn(sqlx::Error) -> AppError {
    move |e| {
        if e.as_database_error().is_some_and(|d| d.is_unique_violation()) {
            AppError::Validation(format!("A {what} with that name already exists."))
        } else {
            e.into()
        }
    }
}

pub fn catalog() -> Value {
    json!(AREAS.iter().map(|a| json!({
        "key": a.key, "label": a.label, "group": a.group,
        "actions": if a.delete { json!(["view", "manage", "delete"]) } else { json!(["view", "manage"]) },
    })).collect::<Vec<_>>())
}

// ── age groups ─────────────────────────────────────────────────────────────────────────────────────
async fn check_age_group(state: &SharedState, foc: Uuid, except: Option<Uuid>, r: &AgeGroupRequest) -> AppResult<String> {
    let name = clean(&r.name, "Age group")?;
    if r.min_age < 0 || r.min_age > 120 || r.max_age.is_some_and(|m| m < r.min_age || m > 130) {
        return Err(AppError::Validation("Ages must be between 0 and 130, and the upper age cannot be below the lower age.".into()));
    }
    let top = r.max_age.unwrap_or(i32::MAX);
    for (other, lo, hi) in repo::age_ranges(&state.db, foc, except).await? {
        if r.min_age <= hi.unwrap_or(i32::MAX) && lo <= top {
            return Err(AppError::Validation(format!("These ages overlap with \"{other}\".")));
        }
    }
    Ok(name)
}

pub async fn age_groups(state: &SharedState, auth: &AuthUser) -> AppResult<Vec<Value>> {
    Ok(repo::age_groups(&state.db, auth.require_foc()?).await?)
}

pub async fn add_age_group(state: &SharedState, auth: &AuthUser, r: AgeGroupRequest) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let name = check_age_group(state, foc, None, &r).await?;
    repo::add_age_group(&state.db, foc, &name, r.min_age, r.max_age).await.map_err(duplicate("age group"))
}

pub async fn update_age_group(state: &SharedState, auth: &AuthUser, id: Uuid, r: AgeGroupRequest) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let name = check_age_group(state, foc, Some(id), &r).await?;
    repo::update_age_group(&state.db, foc, id, &name, r.min_age, r.max_age).await.map_err(duplicate("age group"))?
        .ok_or_else(|| AppError::Validation("Age group not found.".into()))
}

pub async fn delete_age_group(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<()> {
    let foc = auth.require_foc()?;
    if !repo::delete_age_group(&state.db, foc, id).await? {
        return Err(AppError::Validation("Age group not found.".into()));
    }
    state.clear_auth_cache();
    Ok(())
}

// ── roles ──────────────────────────────────────────────────────────────────────────────────────────
async fn check_role(state: &SharedState, foc: Uuid, r: &RoleRequest) -> AppResult<(String, Vec<String>)> {
    let name = clean(&r.name, "Role")?;
    if r.description.as_deref().is_some_and(|d| d.chars().count() > 300) {
        return Err(AppError::Validation("Description is too long (max 300 characters).".into()));
    }
    let perms = normalise(&r.permissions).map_err(AppError::Validation)?;
    if perms.is_empty() {
        return Err(AppError::Validation("Choose at least one permission.".into()));
    }
    let mut cells = r.scope_cell_group_ids.clone(); cells.sort(); cells.dedup();
    let mut ages = r.scope_age_group_ids.clone(); ages.sort(); ages.dedup();
    if repo::count_cell_groups_in(&state.db, foc, &cells).await? != cells.len() as i64
        || repo::count_age_groups_in(&state.db, foc, &ages).await? != ages.len() as i64
    {
        return Err(AppError::Validation("A cell group or age group in the limit does not belong to this church.".into()));
    }
    Ok((name, perms))
}

pub async fn roles(state: &SharedState, auth: &AuthUser) -> AppResult<Vec<Value>> {
    Ok(repo::roles(&state.db, auth.require_foc()?).await?)
}

pub async fn add_role(state: &SharedState, auth: &AuthUser, r: RoleRequest) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let (name, perms) = check_role(state, foc, &r).await?;
    repo::add_role(&state.db, foc, &name, r.description.as_deref(), &perms, &r.scope_cell_group_ids, &r.scope_age_group_ids)
        .await.map_err(duplicate("role"))
}

pub async fn update_role(state: &SharedState, auth: &AuthUser, id: Uuid, r: RoleRequest) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let (name, perms) = check_role(state, foc, &r).await?;
    let out = repo::update_role(&state.db, foc, id, &name, r.description.as_deref(), &perms, &r.scope_cell_group_ids, &r.scope_age_group_ids)
        .await.map_err(duplicate("role"))?
        .ok_or_else(|| AppError::Validation("Role not found.".into()))?;
    state.clear_auth_cache();
    Ok(out)
}

pub async fn delete_role(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<()> {
    let foc = auth.require_foc()?;
    if !repo::delete_role(&state.db, foc, id).await? {
        return Err(AppError::Validation("Role not found.".into()));
    }
    state.clear_auth_cache();
    Ok(())
}

pub async fn role_members(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Vec<Value>> {
    Ok(repo::role_members(&state.db, auth.require_foc()?, id).await?)
}

pub async fn member_roles(state: &SharedState, auth: &AuthUser, member: Uuid) -> AppResult<Vec<Value>> {
    let foc = auth.require_foc()?;
    member_repository::get_json(&state.db, member, foc).await?.ok_or(MemberError::NotFound)?;
    Ok(repo::member_roles(&state.db, foc, member).await?)
}

pub async fn set_member_roles(state: &SharedState, auth: &AuthUser, member: Uuid, r: AssignRolesRequest) -> AppResult<Vec<Value>> {
    let foc = auth.require_foc()?;
    let m = member_repository::get_json(&state.db, member, foc).await?.ok_or(MemberError::NotFound)?;
    if m["user_id"].is_null() {
        return Err(AppError::Validation("That person has no login yet, so they cannot be given a role.".into()));
    }
    if m["status"] != "approved" {
        return Err(AppError::Validation("Only approved members can be given a role.".into()));
    }
    let mut ids = r.role_ids; ids.sort(); ids.dedup();
    if ids.len() > 10 || repo::count_roles_in(&state.db, foc, &ids).await? != ids.len() as i64 {
        return Err(AppError::Validation("Choose up to 10 roles from this church.".into()));
    }
    repo::set_member_roles(&state.db, member, auth.member_id, &ids).await?;
    state.clear_auth_cache();
    Ok(repo::member_roles(&state.db, foc, member).await?)
}

// ── member scope (used by the members service) ─────────────────────────────────────────────────────
/// Limit that applies to this person when they look at members. None = everyone in the church.
pub async fn members_scope(state: &SharedState, auth: &AuthUser) -> AppResult<Option<(Vec<Uuid>, Vec<Uuid>)>> {
    if auth.is_admin() {
        return Ok(None);
    }
    Ok(repo::member_scope(&state.db, auth.member_id, auth.require_foc()?, "members.view").await?)
}

pub async fn ensure_member_in_scope(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<()> {
    if let Some((cells, ages)) = members_scope(state, auth).await? {
        if !repo::in_scope(&state.db, id, auth.require_foc()?, &cells, &ages).await? {
            return Err(AuthError::Forbidden.into());
        }
    }
    Ok(())
}
