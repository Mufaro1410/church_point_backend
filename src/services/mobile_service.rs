//! The member (mobile) API: what a signed-in church member can see and do.

use chrono::{Datelike, Duration, NaiveDate, Utc};
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::{
    errors::{AppError, AppResult, AuthError, MemberError},
    middleware::authentication::AuthUser,
    models::mobile::*,
    repositories::{announcement_repository, event_repository, member_repository, mobile_repository as repo, payment_repository, prayer_repository, testimony_repository},
    state::SharedState,
    utils::dynamic::{self, Kind},
};

const RELATIONS: [&str; 9] = ["spouse", "parent", "child", "sibling", "grandparent", "grandchild", "relative", "guardian", "other"];

/// What a member may change about themselves. Church, role and status are never in here.
const SELF_FIELDS: &[(&str, Kind)] = &[
    ("first_name", Kind::TextReq),
    ("last_name", Kind::TextReq),
    ("gender", Kind::TextReq),
    ("phone", Kind::TextReq),
    ("marital_status", Kind::Text),
    ("profile_image", Kind::Text),
    ("address", Kind::Text),
    ("profession", Kind::Text),
    ("dob", Kind::Date),
    ("date_joined", Kind::Date),
    ("baptism_date", Kind::Date),
    ("cell_group_id", Kind::Uuid),
    ("dbr_plan", Kind::Uuid),
    ("course_ids", Kind::TextArr),
    ("ministry_ids", Kind::UuidArr),
];

fn forget(state: &SharedState) {
    state.clear_auth_cache();
}

// ── public ──

pub async fn public_focs(state: &SharedState) -> AppResult<Vec<Value>> {
    Ok(repo::public_focs(&state.db).await?)
}

// ── everything the app needs to open, in one round trip ──

pub async fn get_notifications(state: &SharedState, auth: &AuthUser) -> AppResult<Value> {
    Ok(repo::notification_prefs(&state.db, auth.member_id).await?)
}

pub async fn set_notifications(state: &SharedState, auth: &AuthUser, r: NotificationPrefsRequest) -> AppResult<Value> {
    Ok(repo::set_notification_prefs(&state.db, auth.member_id, &r).await?)
}

pub async fn bootstrap(state: &SharedState, auth: &AuthUser) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let me = auth.member_id;
    let db = &state.db;
    let today = Utc::now().date_naive();
    let year = today.year();
    let (y0, y1) = (NaiveDate::from_ymd_opt(year, 1, 1).unwrap(), NaiveDate::from_ymd_opt(year, 12, 31).unwrap());

    // Independent queries run side by side (matters when the database is far away).
    let (foc_row, events, prayers, sermons, ministries, leaders, groups, courses, plans, progress, announcements, testimonies, incomes, payments, birthdays, capacities, prefs) = tokio::try_join!(
        repo::foc(db, foc),
        event_repository::list_visible(db, foc, y0, y1),
        prayer_repository::list(db, foc, me, today - Duration::days(365), today + Duration::days(30)),
        repo::sermons(db, foc),
        repo::ministries(db, foc),
        repo::ministry_leaders(db, foc),
        repo::cell_groups(db, foc),
        repo::courses(db, auth.church_id),
        repo::reading_plans(db),
        repo::reading_progress(db, me),
        announcement_repository::list_visible(db, auth, true),
        testimony_repository::list(db, foc, me, None, 300),
        repo::my_incomes(db, foc, me),
        payment_repository::list_for_member(db, me, 30),
        repo::birthdays(db, foc),
        crate::repositories::capacity_repository::list(db, foc),
        repo::notification_prefs(db, me),
    )?;

    // Members never need the check-in QR image or the admins' manual headcount.
    let events: Vec<Value> = events.into_iter().map(|mut e| {
        if let Value::Object(m) = &mut e {
            for k in ["qr_code", "qr_generated_at", "manual_stats"] { m.remove(k); }
        }
        e
    }).collect();
    // Others' testimonies are visible only once approved.
    let testimonies: Vec<Value> = testimonies.into_iter()
        .filter(|t| t["status"] == "approved" || t["is_mine"] == true).collect();

    Ok(json!({
        "foc": foc_row,
        "events": events,
        "prayers": prayers.into_iter().take(100).collect::<Vec<_>>(),
        "sermons": sermons,
        "ministries": ministries,
        "ministry_leaders": leaders,
        "cell_groups": groups,
        "courses": courses,
        "bible_reading_plans": plans,
        "bible_reading_progress": progress,
        "announcements": announcements.into_iter().take(25).collect::<Vec<_>>(),
        "testimonies": testimonies,
        "incomes": incomes,
        "payments": payments,
        "birthdays": birthdays,
        "serving_capacities": capacities,
        "notification_prefs": prefs,
    }))
}

// ── me ──

pub async fn update_profile(state: &SharedState, auth: &AuthUser, body: Map<String, Value>) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let vals = dynamic::parse(body, SELF_FIELDS)?;
    if vals.is_empty() {
        return Err(AppError::Validation("Nothing to change.".into()));
    }
    if let Some(g) = dynamic::get_uuid(&vals, "cell_group_id") {
        if crate::repositories::org_repository::get_group(&state.db, g, foc).await?.is_none() {
            return Err(crate::errors::OrgError::CellGroupNotFound.into());
        }
    }
    member_repository::update_member(&state.db, auth.member_id, foc, vals).await?.ok_or_else(|| MemberError::NotFound.into())
}

pub async fn choose_church(state: &SharedState, auth: &AuthUser, r: ChooseChurch) -> AppResult<()> {
    if repo::choose_church(&state.db, auth.member_id, r.foc_id).await? == 0 {
        return Err(AppError::Validation("You can only pick an approved church, and only if you are not in one already.".into()));
    }
    forget(state);
    Ok(())
}

pub async fn request_transfer(state: &SharedState, auth: &AuthUser) -> AppResult<()> {
    if repo::change_status(&state.db, auth.member_id, "approved", "transfer_requested").await? == 0 {
        return Err(AppError::Validation("A transfer can only be requested by an approved member.".into()));
    }
    forget(state);
    Ok(())
}

pub async fn cancel_transfer(state: &SharedState, auth: &AuthUser) -> AppResult<()> {
    if repo::change_status(&state.db, auth.member_id, "transfer_requested", "approved").await? == 0 {
        return Err(AppError::Validation("There is no transfer request to cancel.".into()));
    }
    forget(state);
    Ok(())
}

pub async fn push_token(state: &SharedState, auth: &AuthUser, r: PushToken) -> AppResult<()> {
    let t = r.token.trim();
    if t.is_empty() || t.len() > 300 {
        return Err(AppError::Validation("token is invalid.".into()));
    }
    Ok(repo::save_push_token(&state.db, auth.member_id, t).await?)
}

// ── events ──

async fn visible_event(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<()> {
    auth.require_foc()?;
    event_repository::find_visible(&state.db, id, auth).await?.ok_or(crate::errors::EventError::NotFound)?;
    Ok(())
}

pub async fn rsvp(state: &SharedState, auth: &AuthUser, id: Uuid, r: RsvpRequest) -> AppResult<Value> {
    if !["going", "maybe", "notGoing"].contains(&r.status.as_str()) {
        return Err(AppError::Validation("status must be going, maybe or notGoing.".into()));
    }
    visible_event(state, auth, id).await?;
    Ok(repo::set_rsvp(&state.db, id, auth.member_id, &r.status).await?)
}

pub async fn check_in(state: &SharedState, auth: &AuthUser, id: Uuid, member: Option<Uuid>) -> AppResult<Value> {
    visible_event(state, auth, id).await?;
    let who = match member {
        Some(m) if m != auth.member_id => {
            let foc = auth.require_foc()?;
            if !repo::is_family(&state.db, auth.member_id, m, foc).await? {
                return Err(AppError::Validation("You can only check in family members linked to your profile.".into()));
            }
            m
        }
        _ => auth.member_id,
    };
    repo::check_in(&state.db, id, who).await?
        .ok_or_else(|| AppError::Validation("Check-in is only open on the day of the event.".into()))
}

pub async fn serving(state: &SharedState, auth: &AuthUser, id: Uuid, r: ServingRequest) -> AppResult<Value> {
    visible_event(state, auth, id).await?;
    let raw: Vec<(Option<Uuid>, String)> = match (r.servings, r.capacity) {
        (Some(list), None) if r.ministry_id.is_none() => list.into_iter().map(|i| (i.ministry_id, i.capacity)).collect(),
        (None, Some(c)) => vec![(r.ministry_id, c)],
        _ => return Err(AppError::Validation("Send either servings, or capacity (with ministry_id).".into())),
    };
    let mut items: Vec<(Option<Uuid>, String)> = Vec::new();
    for (ministry, c) in raw {
        let c = c.trim().to_string();
        if c.is_empty() || c.len() > 80 {
            return Err(AppError::Validation("capacity is required (max 80 characters).".into()));
        }
        if !items.iter().any(|(_, x)| x.eq_ignore_ascii_case(&c)) {
            items.push((ministry, c));
        }
    }
    if items.len() > 10 {
        return Err(AppError::Validation("You can serve in at most 10 places at once.".into()));
    }
    Ok(json!({ "servings": repo::replace_serving(&state.db, id, auth.member_id, &items).await? }))
}

/// Works for a member with no church: their own details, so they can pick a church with the form filled in.
pub async fn my_profile(state: &SharedState, auth: &AuthUser) -> AppResult<Value> {
    repo::my_profile(&state.db, auth.member_id).await?.ok_or_else(|| MemberError::NotFound.into())
}

/// Anonymises the account. Giving records stay (with the payer's name) because the church must keep its books.
pub async fn delete_account(state: &SharedState, auth: &AuthUser, r: DeleteAccountRequest) -> AppResult<()> {
    let reason = r.reason.trim();
    if reason.chars().count() < 3 || reason.chars().count() > 500 {
        return Err(AppError::Validation("Tell us why you are leaving (3 to 500 characters).".into()));
    }
    if auth.is_admin() {
        return Err(AppError::Validation("Church administrators must hand over their role before deleting their account.".into()));
    }
    let Some(login) = repo::delete_account(&state.db, auth.member_id, reason).await? else {
        return Err(MemberError::NotFound.into());
    };
    forget(state);
    if let Some(user) = login {
        match state.supabase.delete_user(user).await {
            Ok(()) => { let _ = repo::login_removed(&state.db, auth.member_id).await; }
            Err(e) => tracing::error!(error = ?e, member = %auth.member_id, "account anonymised but the login could not be removed"),
        }
    }
    Ok(())
}

// ── reactions ──

pub async fn react_prayer(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let r = repo::toggle_prayer_reaction(&state.db, id, foc, auth.member_id).await?.ok_or(crate::errors::PrayerError::NotFound)?;
    Ok(json!({ "reactions": r }))
}

pub async fn react_testimony(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let r = repo::toggle_testimony_reaction(&state.db, id, foc, auth.member_id).await?.ok_or(crate::errors::TestimonyError::NotFound)?;
    Ok(json!({ "reactions": r }))
}

// ── sermons, reading ──

pub async fn sermon_content(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let c = repo::sermon_content(&state.db, id, foc).await?.ok_or(crate::errors::SermonError::NotFound)?;
    Ok(json!({ "content": c }))
}

pub async fn save_progress(state: &SharedState, auth: &AuthUser, plan: Uuid, r: ProgressRequest) -> AppResult<Value> {
    if !r.progress.is_object() || r.progress.to_string().len() > 100_000 {
        return Err(AppError::Validation("progress must be an object.".into()));
    }
    if !repo::plan_exists(&state.db, plan).await? {
        return Err(AppError::Validation("Unknown reading plan.".into()));
    }
    Ok(repo::merge_progress(&state.db, auth.member_id, plan, &r.progress).await?)
}

pub async fn get_progress(state: &SharedState, auth: &AuthUser, plan: Uuid) -> AppResult<Value> {
    Ok(repo::progress_for(&state.db, auth.member_id, plan).await?.unwrap_or(Value::Null))
}

// ── family ──

pub async fn my_family(state: &SharedState, auth: &AuthUser) -> AppResult<Vec<Value>> {
    Ok(repo::my_family(&state.db, auth.member_id, auth.require_foc()?).await?)
}

pub async fn search_members(state: &SharedState, auth: &AuthUser, q: SearchQuery) -> AppResult<Vec<Value>> {
    let foc = auth.require_foc()?;
    let text = q.q.unwrap_or_default();
    let text = text.trim();
    if text.chars().count() < 2 {
        return Ok(vec![]);
    }
    let esc = text.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    Ok(repo::search_members(&state.db, foc, auth.member_id, &format!("%{esc}%")).await?)
}

pub async fn add_family(state: &SharedState, auth: &AuthUser, r: AddFamily) -> AppResult<()> {
    let foc = auth.require_foc()?;
    if !RELATIONS.contains(&r.relationship_type.as_str()) {
        return Err(AppError::Validation(format!("relationship_type must be one of: {}.", RELATIONS.join(", "))));
    }
    if r.related_member_id == auth.member_id || !repo::member_in_foc(&state.db, r.related_member_id, foc).await? {
        return Err(MemberError::NotFound.into());
    }
    repo::add_family(&state.db, auth.member_id, r.related_member_id, &r.relationship_type).await?;
    Ok(())
}

pub async fn remove_family(state: &SharedState, auth: &AuthUser, related: Uuid) -> AppResult<()> {
    repo::remove_family(&state.db, auth.member_id, related).await?;
    Ok(())
}

/// Only people the church has accepted may use the member API.
pub fn require_active(auth: &AuthUser) -> Result<(), AuthError> {
    match auth.member_status.as_deref() {
        Some("approved" | "transfer_requested") if auth.foc_id.is_some() => Ok(()),
        _ => Err(AuthError::Forbidden),
    }
}
