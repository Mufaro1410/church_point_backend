use serde_json::{Map, Value};
use uuid::Uuid;

use crate::{
    errors::{AppError, AppResult, AuthError, MemberError, OrgError},
    middleware::authentication::AuthUser,
    models::catalog::*,
    repositories::{catalog_repository as repo, member_repository, org_repository},
    services::event_service,
    state::SharedState,
    utils::{
        dynamic::{self, Kind, Val},
        validation::{optional_line, required_line},
    },
};

fn super_only(auth: &AuthUser) -> AppResult<()> {
    if auth.is_super_admin() { Ok(()) } else { Err(AuthError::Forbidden.into()) }
}

// ── ministries (shared catalogue: FOC admins add + select; only the platform owner renames/deletes) ──

pub async fn ministries(state: &SharedState, auth: &AuthUser) -> AppResult<Vec<Value>> {
    Ok(repo::list_ministries(&state.db, auth.require_foc()?).await?)
}

pub async fn add_ministry(state: &SharedState, auth: &AuthUser, r: NameRequest) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let name = required_line("name", r.name.as_deref().unwrap_or(""), 80)?;
    let desc = optional_line("description", r.description, 500)?;
    if repo::ministry_name_taken(&state.db, &name, None).await? {
        return Err(OrgError::Duplicate("A ministry with this name").into());
    }
    Ok(repo::insert_ministry(&state.db, foc, &name, desc.as_deref()).await?)
}

pub async fn select_ministry(state: &SharedState, auth: &AuthUser, id: Uuid, selected: bool) -> AppResult<()> {
    let foc = auth.require_foc()?;
    if !repo::ministry_exists(&state.db, id).await? {
        return Err(OrgError::NotFound("Ministry").into());
    }
    Ok(repo::set_ministry_selected(&state.db, foc, id, selected).await?)
}

pub async fn set_leader(state: &SharedState, auth: &AuthUser, id: Uuid, leader: Option<Uuid>) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    if !repo::ministry_selected(&state.db, foc, id).await? {
        return Err(AppError::Validation("Enable this ministry for your FOC before assigning a leader.".into()));
    }
    if let Some(l) = leader {
        if !org_repository::leader_in_foc(&state.db, l, foc).await? {
            return Err(OrgError::LeaderNotInFoc.into());
        }
    }
    Ok(repo::set_ministry_leader(&state.db, foc, id, leader).await?)
}

pub async fn rename_ministry(state: &SharedState, auth: &AuthUser, id: Uuid, r: NameRequest) -> AppResult<Value> {
    super_only(auth)?;
    let name = r.name.map(|n| required_line("name", &n, 80)).transpose()?;
    let desc = optional_line("description", r.description, 500)?;
    if let Some(n) = &name {
        if repo::ministry_name_taken(&state.db, n, Some(id)).await? {
            return Err(OrgError::Duplicate("A ministry with this name").into());
        }
    }
    repo::rename_ministry(&state.db, id, name.as_deref(), desc.as_deref()).await?.ok_or_else(|| OrgError::NotFound("Ministry").into())
}

pub async fn delete_ministry(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<()> {
    super_only(auth)?;
    match repo::delete_ministry(&state.db, id).await? {
        0 => Err(OrgError::NotFound("Ministry").into()),
        _ => Ok(()),
    }
}

// ── courses (belong to the church) ──

pub async fn courses(state: &SharedState, auth: &AuthUser) -> AppResult<Vec<Value>> {
    auth.require_foc()?;
    let church = auth.church_id.ok_or(AuthError::NoFoc)?;
    Ok(repo::list_courses(&state.db, church).await?)
}

pub async fn add_course(state: &SharedState, auth: &AuthUser, r: NameRequest) -> AppResult<Value> {
    auth.require_foc()?;
    let church = auth.church_id.ok_or(AuthError::NoFoc)?;
    let name = required_line("name", r.name.as_deref().unwrap_or(""), 80)?;
    let desc = optional_line("description", r.description, 500)?;
    if repo::course_name_taken(&state.db, church, &name, None).await? {
        return Err(OrgError::Duplicate("A course with this name").into());
    }
    Ok(repo::insert_course(&state.db, church, &name, desc.as_deref()).await?)
}

pub async fn rename_course(state: &SharedState, auth: &AuthUser, id: Uuid, r: NameRequest) -> AppResult<Value> {
    super_only(auth)?;
    let name = r.name.map(|n| required_line("name", &n, 80)).transpose()?;
    let desc = optional_line("description", r.description, 500)?;
    repo::rename_course(&state.db, id, name.as_deref(), desc.as_deref()).await?.ok_or_else(|| OrgError::NotFound("Course").into())
}

pub async fn delete_course(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<()> {
    super_only(auth)?;
    match repo::delete_course(&state.db, id).await? {
        0 => Err(OrgError::NotFound("Course").into()),
        _ => Ok(()),
    }
}

// ── who serves at an event ──

pub async fn list_serving(state: &SharedState, auth: &AuthUser, event: Uuid) -> AppResult<Vec<Value>> {
    let foc = auth.require_foc()?;
    event_service::get(state, auth, event).await?;
    Ok(repo::list_serving(&state.db, event, foc).await?)
}

pub async fn all_serving(state: &SharedState, auth: &AuthUser) -> AppResult<Vec<Value>> {
    Ok(repo::list_all_serving(&state.db, auth.require_foc()?).await?)
}

pub async fn add_serving(state: &SharedState, auth: &AuthUser, event: Uuid, r: ServingRequest) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    event_service::get(state, auth, event).await?;
    member_repository::find_in_foc(&state.db, r.profile_id, foc).await?.ok_or(MemberError::NotFound)?;
    if let Some(m) = r.ministry_id {
        if !repo::ministry_exists(&state.db, m).await? {
            return Err(OrgError::NotFound("Ministry").into());
        }
    }
    let cap = optional_line("capacity", r.capacity, 100)?;
    Ok(repo::insert_serving(&state.db, event, r.profile_id, r.ministry_id, cap.as_deref()).await?)
}

pub async fn remove_serving(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<()> {
    match repo::delete_serving(&state.db, id, auth.require_foc()?).await? {
        0 => Err(OrgError::NotFound("Serving record").into()),
        _ => Ok(()),
    }
}

// ── social events ──

const SOCIAL: &[(&str, Kind)] = &[
    ("name", Kind::TextReq),
    ("type", Kind::TextReq),
    ("date", Kind::Date),
    ("description", Kind::Text),
    ("location", Kind::Text),
    ("time", Kind::Time),
    ("is_active", Kind::Bool),
];

fn check_social_type(vals: &[(&'static str, Val)]) -> AppResult<()> {
    for (n, v) in vals {
        if let ("type", Val::Text(Some(t))) = (*n, v) {
            if !["Birthday", "Anniversary", "Memorial", "Other"].contains(&t.as_str()) {
                return Err(AppError::Validation("type must be Birthday, Anniversary, Memorial or Other.".into()));
            }
        }
    }
    Ok(())
}

pub async fn social_list(state: &SharedState, auth: &AuthUser) -> AppResult<Vec<Value>> {
    Ok(repo::list_social(&state.db, auth.require_foc()?).await?)
}

pub async fn social_create(state: &SharedState, auth: &AuthUser, body: Map<String, Value>) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let vals = dynamic::parse(body, SOCIAL)?;
    for f in ["name", "type", "date"] {
        if !dynamic::has(&vals, f) {
            return Err(AppError::Validation(format!("{f} is required.")));
        }
    }
    check_social_type(&vals)?;
    Ok(repo::insert_row(&state.db, "social_events", vals,
        vec![("foc_id", Val::Uuid(Some(foc))), ("created_by", Val::Uuid(Some(auth.user_id)))]).await?)
}

pub async fn social_update(state: &SharedState, auth: &AuthUser, id: Uuid, body: Map<String, Value>) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let vals = dynamic::parse(body, SOCIAL)?;
    if vals.is_empty() {
        return Err(AppError::Validation("Nothing to update.".into()));
    }
    check_social_type(&vals)?;
    repo::update_row(&state.db, "social_events", id, "foc_id", foc, "", vals).await?.ok_or_else(|| OrgError::NotFound("Social event").into())
}

pub async fn social_delete(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<()> {
    match repo::delete_row(&state.db, "social_events", id, "foc_id", auth.require_foc()?).await? {
        0 => Err(OrgError::NotFound("Social event").into()),
        _ => Ok(()),
    }
}

// ── bible reading plans (shared library; only the author changes or removes a plan) ──

const PLAN: &[(&str, Kind)] = &[
    ("name", Kind::TextReq),
    ("year", Kind::Int),
    ("description", Kind::Text),
    ("readings", Kind::Json),
];

fn check_readings(vals: &[(&'static str, Val)]) -> AppResult<()> {
    for (n, v) in vals {
        if *n == "readings" && !matches!(v, Val::Json(Some(Value::Array(a))) if !a.is_empty()) {
            return Err(AppError::Validation("readings must be a non-empty list.".into()));
        }
    }
    Ok(())
}

pub async fn plans(state: &SharedState, auth: &AuthUser) -> AppResult<Vec<Value>> {
    Ok(repo::list_plans(&state.db, auth.user_id).await?)
}

pub async fn plan_create(state: &SharedState, auth: &AuthUser, body: Map<String, Value>) -> AppResult<Value> {
    let vals = dynamic::parse(body, PLAN)?;
    for f in ["name", "year", "readings"] {
        if !dynamic::has(&vals, f) {
            return Err(AppError::Validation(format!("{f} is required.")));
        }
    }
    check_readings(&vals)?;
    Ok(repo::insert_row(&state.db, "bible_reading_plans", vals, vec![("user_id", Val::Uuid(Some(auth.user_id)))]).await?)
}

pub async fn plan_update(state: &SharedState, auth: &AuthUser, id: Uuid, body: Map<String, Value>) -> AppResult<Value> {
    let vals = dynamic::parse(body, PLAN)?;
    if vals.is_empty() {
        return Err(AppError::Validation("Nothing to update.".into()));
    }
    check_readings(&vals)?;
    repo::update_row(&state.db, "bible_reading_plans", id, "user_id", auth.user_id, "deleted_at IS NULL", vals)
        .await?
        .ok_or_else(|| OrgError::NotFound("Plan").into())
}

pub async fn plan_delete(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<()> {
    match repo::soft_delete_plan(&state.db, id, auth.user_id).await? {
        0 => Err(OrgError::NotFound("Plan").into()),
        _ => Ok(()),
    }
}

// ── church, FOCs, FOC settings ──

pub async fn church(state: &SharedState, auth: &AuthUser) -> AppResult<Value> {
    auth.require_foc()?;
    let id = auth.church_id.ok_or(AuthError::NoFoc)?;
    repo::church(&state.db, id).await?.ok_or_else(|| OrgError::NotFound("Church").into())
}

pub async fn focs(state: &SharedState, auth: &AuthUser) -> AppResult<Vec<Value>> {
    auth.require_foc()?;
    let id = auth.church_id.ok_or(AuthError::NoFoc)?;
    Ok(repo::focs_in_church(&state.db, id).await?)
}

const FOC_FIELDS: &[(&str, Kind)] = &[
    ("name", Kind::TextReq),
    ("address", Kind::TextReq),
    ("email", Kind::TextReq),
    ("logo", Kind::TextReq),
    ("contact", Kind::Text),
    ("country", Kind::Text),
    ("about", Kind::Text),
    ("website", Kind::Text),
    ("service_time", Kind::Text),
    ("social_media", Kind::TextArr),
    ("payment_details", Kind::Json),
];

pub async fn update_foc(state: &SharedState, auth: &AuthUser, body: Map<String, Value>) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let vals = dynamic::parse(body, FOC_FIELDS)?;
    if vals.is_empty() {
        return Err(AppError::Validation("Nothing to update.".into()));
    }
    repo::update_row(&state.db, "foc_profile", foc, "id", foc, "", vals).await?.ok_or_else(|| crate::errors::FocError::NotFound.into())
}

pub fn clean_register(r: RegisterFoc) -> AppResult<(RegisterFoc, Option<String>)> {
    let c = RegisterFoc {
        church_data: ChurchData {
            name: required_line("name", &r.church_data.name, 120)?,
            address: required_line("address", &r.church_data.address, 300)?,
            email: required_line("email", &r.church_data.email, 200)?,
            contact: required_line("contact", &r.church_data.contact, 50)?,
            ..r.church_data
        },
        user_data: UserData {
            first_name: required_line("first_name", &r.user_data.first_name, 80)?,
            last_name: required_line("last_name", &r.user_data.last_name, 80)?,
            email: required_line("email", &r.user_data.email, 200)?,
            phone: required_line("phone", &r.user_data.phone, 50)?,
            gender: required_line("gender", &r.user_data.gender, 20)?,
            ..r.user_data
        },
    };
    let dob = match c.user_data.dob.as_deref().filter(|d| !d.is_empty()) {
        Some(d) => {
            let d = d.get(..10).unwrap_or(d);
            chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").map_err(|_| AppError::Validation("dob must look like 1990-05-30.".into()))?;
            Some(d.to_string())
        }
        None => None,
    };
    Ok((c, dob))
}

pub async fn register_foc(state: &SharedState, user_id: Uuid, r: RegisterFoc) -> AppResult<Value> {
    let (c, dob) = clean_register(r)?;
    register_cleaned(state, user_id, &c, dob.as_deref()).await
}

pub async fn register_cleaned(state: &SharedState, user_id: Uuid, c: &RegisterFoc, dob: Option<&str>) -> AppResult<Value> {
    if !repo::church_exists(&state.db, c.church_data.church_id).await? {
        return Err(AppError::Validation("church_id does not match a church.".into()));
    }
    if repo::profile_exists_for_user(&state.db, user_id).await? {
        return Err(OrgError::Duplicate("A profile for this account").into());
    }
    Ok(repo::register_foc(&state.db, user_id, &c.church_data, &c.user_data, dob).await?)
}

// ── event services (who served in which role) ──

pub async fn list_services(state: &SharedState, auth: &AuthUser, event: Uuid) -> AppResult<Vec<Value>> {
    let foc = auth.require_foc()?;
    event_service::get(state, auth, event).await?;
    Ok(repo::list_services(&state.db, event, foc).await?)
}

pub async fn add_service(state: &SharedState, auth: &AuthUser, event: Uuid, r: ServiceRequest) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    event_service::get(state, auth, event).await?;
    member_repository::find_in_foc(&state.db, r.member_id, foc).await?.ok_or(MemberError::NotFound)?;
    let role = required_line("service_role", &r.service_role, 80)?;
    let domain = optional_line("service_domain", r.service_domain, 80)?;
    let notes = optional_line("notes", r.notes, 500)?;
    Ok(repo::insert_service(&state.db, event, r.member_id, &role, domain.as_deref(), notes.as_deref(), auth.member_id).await?)
}

pub async fn remove_service(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<()> {
    match repo::delete_service(&state.db, id, auth.require_foc()?).await? {
        0 => Err(OrgError::NotFound("Service record").into()),
        _ => Ok(()),
    }
}

// ── logos (files in our storage bucket) ──

pub async fn logos(state: &SharedState, auth: &AuthUser) -> AppResult<Vec<String>> {
    auth.require_foc()?;
    state.supabase.list_logos().await
}
