//! Visitors and follow-up: first-time guests, a contact log, and members who stopped coming.

use serde_json::Value;
use uuid::Uuid;

use crate::{
    errors::{AppError, AppResult},
    middleware::authentication::AuthUser,
    repositories::visitor_repository::{self as repo, VisitorIn},
    state::SharedState,
};

const STATUSES: [&str; 5] = ["new", "contacted", "following_up", "joined", "closed"];
const KINDS: [&str; 4] = ["call", "visit", "message", "other"];

fn bad(m: &str) -> AppError { AppError::Validation(m.into()) }
fn missing() -> AppError { bad("Visitor not found.") }

fn tidy(v: Option<String>, max: usize, what: &str) -> AppResult<Option<String>> {
    match v.map(|s| s.trim().to_string()) {
        None => Ok(None),
        Some(s) if s.chars().count() > max => Err(AppError::Validation(format!("{what} is too long."))),
        Some(s) => Ok(Some(s)),
    }
}

async fn clean(state: &SharedState, foc: Uuid, mut v: VisitorIn, creating: bool) -> AppResult<VisitorIn> {
    v.full_name = tidy(v.full_name, 120, "Name")?;
    if creating && v.full_name.as_deref().is_none_or(str::is_empty) { return Err(bad("Name is required.")); }
    if v.full_name.as_deref() == Some("") { return Err(bad("Name cannot be empty.")); }
    v.phone = tidy(v.phone, 40, "Phone")?;
    v.email = tidy(v.email, 200, "Email")?;
    v.how_heard = tidy(v.how_heard, 200, "How they heard")?;
    v.notes = tidy(v.notes, 4000, "Notes")?;
    if let Some(s) = &v.status && !STATUSES.contains(&s.as_str()) { return Err(bad("Unknown status.")); }
    if let Some(m) = v.assigned_to && !repo::member_in_foc(&state.db, foc, m).await? { return Err(bad("Assignee must be a member of this church.")); }
    Ok(v)
}

pub async fn list(state: &SharedState, auth: &AuthUser, status: Option<&str>) -> AppResult<Vec<Value>> {
    Ok(repo::list(&state.db, auth.require_foc()?, status).await?)
}

pub async fn get(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Value> {
    repo::get(&state.db, auth.require_foc()?, id).await?.ok_or_else(missing)
}

pub async fn add(state: &SharedState, auth: &AuthUser, v: VisitorIn) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let v = clean(state, foc, v, true).await?;
    let id = repo::insert(&state.db, foc, auth.member_id, &v).await?;
    get(state, auth, id).await
}

pub async fn update(state: &SharedState, auth: &AuthUser, id: Uuid, v: VisitorIn) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let v = clean(state, foc, v, false).await?;
    if !repo::update(&state.db, foc, id, &v).await? { return Err(missing()); }
    get(state, auth, id).await
}

pub async fn remove(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<()> {
    if repo::remove(&state.db, auth.require_foc()?, id).await? { Ok(()) } else { Err(missing()) }
}

pub async fn convert(state: &SharedState, auth: &AuthUser, id: Uuid, member: Option<Uuid>) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    if let Some(m) = member && !repo::member_in_foc(&state.db, foc, m).await? { return Err(bad("That person is not a member of this church.")); }
    if !repo::convert(&state.db, foc, id, member).await? { return Err(missing()); }
    get(state, auth, id).await
}

fn kind_ok(kind: Option<String>) -> AppResult<String> {
    let k = kind.unwrap_or_else(|| "call".into());
    if KINDS.contains(&k.as_str()) { Ok(k) } else { Err(bad("Unknown follow-up type.")) }
}

pub async fn visitor_log(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Vec<Value>> {
    let foc = auth.require_foc()?;
    get(state, auth, id).await?;
    Ok(repo::log(&state.db, foc, Some(id), None).await?)
}

pub async fn visitor_log_add(state: &SharedState, auth: &AuthUser, id: Uuid, kind: Option<String>, note: Option<String>) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    get(state, auth, id).await?;
    let note = tidy(note, 4000, "Note")?;
    let row = repo::log_add(&state.db, foc, auth.member_id, Some(id), None, &kind_ok(kind)?, note.as_deref()).await?;
    repo::mark_contacted(&state.db, foc, id).await?;
    Ok(row)
}

pub async fn member_log(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Vec<Value>> {
    let foc = auth.require_foc()?;
    if !repo::member_in_foc(&state.db, foc, id).await? { return Err(bad("Member not found.")); }
    Ok(repo::log(&state.db, foc, None, Some(id)).await?)
}

pub async fn member_log_add(state: &SharedState, auth: &AuthUser, id: Uuid, kind: Option<String>, note: Option<String>) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    if !repo::member_in_foc(&state.db, foc, id).await? { return Err(bad("Member not found.")); }
    let note = tidy(note, 4000, "Note")?;
    Ok(repo::log_add(&state.db, foc, auth.member_id, None, Some(id), &kind_ok(kind)?, note.as_deref()).await?)
}

pub async fn absent(state: &SharedState, auth: &AuthUser, weeks: Option<i32>) -> AppResult<Vec<Value>> {
    Ok(repo::absent(&state.db, auth.require_foc()?, weeks.unwrap_or(3).clamp(1, 52)).await?)
}

// ── QR code sign-in form ──

pub async fn form_link(state: &SharedState, auth: &AuthUser) -> AppResult<Value> {
    Ok(repo::form_link(&state.db, auth.require_foc()?).await?)
}

pub async fn set_form_link(state: &SharedState, auth: &AuthUser, enabled: Option<bool>, rotate: bool) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    repo::set_form_link(&state.db, foc, enabled, rotate).await?;
    Ok(repo::form_link(&state.db, foc).await?)
}

/// What the public page shows before the form: only the church's name and logo.
pub async fn public_church(state: &SharedState, token: &str) -> AppResult<Value> {
    let (_, name, logo) = repo::church_for_token(&state.db, token).await?.ok_or_else(|| bad("This form is not available."))?;
    Ok(serde_json::json!({ "church_name": name, "logo": logo }))
}

/// A guest signs themselves in. Quietly ignores bots (honeypot) and repeat submissions; limits floods.
pub async fn public_submit(state: &SharedState, token: &str, v: VisitorIn, honeypot: Option<String>) -> AppResult<()> {
    let (foc, _, _) = repo::church_for_token(&state.db, token).await?.ok_or_else(|| bad("This form is not available."))?;
    if honeypot.is_some_and(|h| !h.trim().is_empty()) { return Ok(()); }
    let v = clean(state, foc, VisitorIn { visit_date: None, status: None, assigned_to: None, ..v }, true).await?;
    if repo::form_submissions_last_hour(&state.db, foc).await? >= 60 {
        return Err(bad("Too many sign-ins right now. Please tell someone at the church."));
    }
    if let Some(phone) = v.phone.as_deref().filter(|p| !p.is_empty()) && repo::recent_duplicate(&state.db, foc, phone).await? { return Ok(()); }
    repo::insert_from_form(&state.db, foc, &v).await?;
    Ok(())
}
