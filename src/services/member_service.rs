use serde_json::{Map, Value};
use uuid::Uuid;

use crate::{
    errors::{AppError, AppResult, MemberError, OrgError},
    middleware::authentication::AuthUser,
    models::member::{FamilyRequest, MemberQuery, ReleaseRequest},
    repositories::{member_repository as repo, member_repository::Filter, org_repository},
    state::SharedState,
    utils::{
        dynamic::{self, Kind, Val},
        pagination::Paginated,
    },
};

/// Fields an admin may set. role / foc_id / user_id / status are deliberately absent.
const FIELDS: &[(&str, Kind)] = &[
    ("first_name", Kind::TextReq),
    ("last_name", Kind::TextReq),
    ("gender", Kind::TextReq),
    ("phone", Kind::TextReq),
    ("marital_status", Kind::Text),
    ("profile_image", Kind::Text),
    ("email", Kind::Text),
    ("address", Kind::Text),
    ("profession", Kind::Text),
    ("membership_status", Kind::Text),
    ("dob", Kind::Date),
    ("date_joined", Kind::Date),
    ("baptism_date", Kind::Date),
    ("cell_group_id", Kind::Uuid),
    ("dbr_plan", Kind::Uuid),
    ("course_ids", Kind::TextArr),
    ("ministry_ids", Kind::UuidArr),
];

const RELATIONS: [&str; 7] = ["spouse", "parent", "child", "sibling", "grandparent", "grandchild", "relative"];
const RELEASE_STATUSES: [&str; 4] = ["approved", "rejected", "pending", "transfer_requested"];
const SET_STATUSES: [&str; 2] = ["approved", "pending"];

fn like(s: &str) -> String {
    let esc = s.trim().replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    format!("%{esc}%")
}

pub async fn list(state: &SharedState, auth: &AuthUser, q: MemberQuery) -> AppResult<Paginated<Value>> {
    let foc_id = auth.require_foc()?;
    let per_page = q.per_page.unwrap_or(500).clamp(1, 1000);
    let page = q.page.unwrap_or(1).max(1);
    let f = Filter {
        status: q.status.filter(|s| !s.is_empty()),
        search: q.search.filter(|s| !s.trim().is_empty()).map(|s| like(&s)),
        cell_group_id: q.cell_group_id,
        age_group_id: q.age_group_id,
        scope: crate::services::rbac_service::members_scope(state, auth).await?,
    };
    let items = repo::list_json(&state.db, foc_id, &f, per_page, (page - 1) * per_page).await?;
    let total = repo::count_filtered(&state.db, foc_id, &f).await?;
    Ok(Paginated { items, total, page, per_page })
}

pub async fn get(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Value> {
    let foc_id = auth.require_foc()?;
    crate::services::rbac_service::ensure_member_in_scope(state, auth, id).await?;
    repo::get_json(&state.db, id, foc_id).await?.ok_or_else(|| MemberError::NotFound.into())
}

/// Business rule: an admin must not remove or reject their own profile.
fn forbid_self(auth: &AuthUser, id: Uuid) -> AppResult<()> {
    if auth.member_id == id {
        return Err(AppError::Validation("You cannot do this to your own profile.".into()));
    }
    Ok(())
}

async fn check_links(state: &SharedState, foc_id: Uuid, vals: &[(&'static str, Val)]) -> AppResult<()> {
    if let Some(g) = dynamic::get_uuid(vals, "cell_group_id") {
        if org_repository::get_group(&state.db, g, foc_id).await?.is_none() {
            return Err(OrgError::CellGroupNotFound.into());
        }
    }
    if let Some(ids) = dynamic::get_uuid_arr(vals, "ministry_ids") {
        if !ids.is_empty() && !repo::ministries_exist(&state.db, ids).await? {
            return Err(AppError::Validation("ministry_ids contains an unknown ministry.".into()));
        }
    }
    Ok(())
}

fn parse_fields(body: Map<String, Value>) -> AppResult<Vec<(&'static str, Val)>> {
    dynamic::parse(body, FIELDS)
}

pub async fn create(state: &SharedState, auth: &AuthUser, body: Map<String, Value>) -> AppResult<Value> {
    if crate::services::rbac_service::members_scope(state, auth).await?.is_some() {
        return Err(crate::errors::AuthError::Forbidden.into());
    }
    let foc_id = auth.require_foc()?;
    let vals = parse_fields(body)?;
    for required in ["first_name", "last_name", "gender", "phone"] {
        if !dynamic::has(&vals, required) {
            return Err(AppError::Validation(format!("{required} is required.")));
        }
    }
    check_links(state, foc_id, &vals).await?;
    Ok(repo::insert_member(&state.db, foc_id, vals).await?)
}

pub async fn update(state: &SharedState, auth: &AuthUser, id: Uuid, body: Map<String, Value>) -> AppResult<Value> {
    crate::services::rbac_service::ensure_member_in_scope(state, auth, id).await?;
    let foc_id = auth.require_foc()?;
    let vals = parse_fields(body)?;
    if vals.is_empty() {
        return Err(AppError::Validation("Nothing to update.".into()));
    }
    check_links(state, foc_id, &vals).await?;
    repo::update_member(&state.db, id, foc_id, vals).await?.ok_or_else(|| MemberError::NotFound.into())
}

pub async fn set_status(state: &SharedState, auth: &AuthUser, id: Uuid, status: &str) -> AppResult<Value> {
    crate::services::rbac_service::ensure_member_in_scope(state, auth, id).await?;
    let foc_id = auth.require_foc()?;
    if !SET_STATUSES.contains(&status) {
        return Err(AppError::Validation("status must be approved or pending.".into()));
    }
    if status == "pending" {
        forbid_self(auth, id)?;
    }
    repo::set_status(&state.db, id, foc_id, status).await?.ok_or_else(|| MemberError::NotFound.into())
}

pub async fn approve(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Value> {
    set_status(state, auth, id, "approved").await
}

pub async fn set_pending(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Value> {
    set_status(state, auth, id, "pending").await
}

pub async fn release(state: &SharedState, auth: &AuthUser, id: Uuid, req: ReleaseRequest) -> AppResult<()> {
    crate::services::rbac_service::ensure_member_in_scope(state, auth, id).await?;
    let foc_id = auth.require_foc()?;
    forbid_self(auth, id)?;
    if !RELEASE_STATUSES.contains(&req.status.as_str()) {
        return Err(AppError::Validation("status must be approved, rejected, pending or transfer_requested.".into()));
    }
    let changed = repo::release(&state.db, id, foc_id, &req.status).await?;
    state.clear_auth_cache();
    match changed {
        0 => Err(MemberError::NotFound.into()),
        _ => Ok(()),
    }
}

/// Rejecting = mark rejected AND release from the FOC, in one statement.
pub async fn reject(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<()> {
    release(state, auth, id, ReleaseRequest { status: "rejected".into() }).await
}

pub async fn delete(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<()> {
    crate::services::rbac_service::ensure_member_in_scope(state, auth, id).await?;
    let foc_id = auth.require_foc()?;
    forbid_self(auth, id)?;
    let changed = repo::soft_delete(&state.db, id, foc_id).await?;
    state.clear_auth_cache();
    match changed {
        0 => Err(MemberError::NotFound.into()),
        _ => Ok(()),
    }
}

pub async fn set_role(state: &SharedState, auth: &AuthUser, id: Uuid, role: &str) -> AppResult<Value> {
    let foc_id = auth.require_foc()?;
    if !["member", "admin"].contains(&role) {
        return Err(AppError::Validation("The built-in roles are member and admin. Other roles are created under Roles and given to people.".into()));
    }
    forbid_self(auth, id)?;
    get(state, auth, id).await?;
    let updated = repo::set_role(&state.db, id, foc_id, role).await?;
    state.clear_auth_cache();
    updated
        .ok_or_else(|| AppError::Validation("That person has no login yet, so they cannot be given this role.".into()))
}

pub async fn family(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Vec<Value>> {
    let foc_id = auth.require_foc()?;
    get(state, auth, id).await?;
    Ok(repo::list_family(&state.db, id, foc_id).await?)
}

pub async fn add_family(state: &SharedState, auth: &AuthUser, id: Uuid, req: FamilyRequest) -> AppResult<Value> {
    let foc_id = auth.require_foc()?;
    if !RELATIONS.contains(&req.relationship_type.as_str()) {
        return Err(AppError::Validation(format!("relationship_type must be one of: {}.", RELATIONS.join(", "))));
    }
    if id == req.related_member_id {
        return Err(AppError::Validation("A member cannot be related to themselves.".into()));
    }
    get(state, auth, id).await?;
    repo::find_in_foc(&state.db, req.related_member_id, foc_id).await?.ok_or(MemberError::NotFound)?;
    Ok(repo::add_relationship(&state.db, id, req.related_member_id, &req.relationship_type).await?)
}

pub async fn remove_family(state: &SharedState, auth: &AuthUser, rel_id: Uuid) -> AppResult<()> {
    match repo::remove_relationship(&state.db, rel_id, auth.require_foc()?).await? {
        0 => Err(MemberError::NotFound.into()),
        _ => Ok(()),
    }
}

pub async fn contributions(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Vec<Value>> {
    let foc_id = auth.require_foc()?;
    get(state, auth, id).await?;
    Ok(repo::contributions(&state.db, id, foc_id).await?)
}

pub async fn attendance(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Vec<Value>> {
    let foc_id = auth.require_foc()?;
    get(state, auth, id).await?;
    Ok(repo::attendance(&state.db, id, foc_id, auth.church_id, auth.province_id).await?)
}

pub async fn activity(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Vec<Value>> {
    let foc_id = auth.require_foc()?;
    get(state, auth, id).await?;
    Ok(repo::activity(&state.db, id, foc_id).await?)
}
