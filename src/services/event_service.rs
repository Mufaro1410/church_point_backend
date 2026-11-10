use std::collections::HashMap;

use chrono::{Datelike, NaiveDate, Utc};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    errors::{AppError, AppResult, AuthError, EventError, MemberError},
    middleware::authentication::AuthUser,
    models::event::{
        AttendanceSummary, Attendee, CreateEventRequest, EventsQuery, ManualStats,
        MarkAttendanceRequest, MemberName, NewEvent, RsvpCategory, RsvpStats, UpdateEventRequest,
    },
    repositories::{event_repository, member_repository},
    state::SharedState,
    utils::{qr, validation::non_empty},
};
use super::scope;

pub async fn list(state: &SharedState, auth: &AuthUser, q: EventsQuery) -> AppResult<Vec<Value>> {
    let foc_id = auth.require_foc()?;
    let year = Utc::now().year();
    let start = q.start.unwrap_or_else(|| NaiveDate::from_ymd_opt(year, 1, 1).unwrap());
    let end = q.end.unwrap_or_else(|| NaiveDate::from_ymd_opt(year, 12, 31).unwrap());
    Ok(event_repository::list_visible(&state.db, foc_id, start, end).await?)
}

pub async fn get(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Value> {
    auth.require_foc()?;
    event_repository::find_visible(&state.db, id, auth)
        .await?
        .ok_or_else(|| EventError::NotFound.into())
}

/// Integrity rule: an event may only be linked to a sermon of the caller's own FOC.
async fn check_sermon(state: &SharedState, sermon_id: Uuid, foc_id: Uuid) -> AppResult<()> {
    if !event_repository::sermon_in_foc(&state.db, sermon_id, foc_id).await? {
        return Err(AppError::Validation("sermon_id does not match a sermon in your FOC.".into()));
    }
    Ok(())
}

fn check_stats(stats: &ManualStats) -> AppResult<()> {
    if !stats.is_valid() {
        return Err(AppError::Validation("manual_stats values cannot be negative.".into()));
    }
    Ok(())
}

pub async fn create(state: &SharedState, auth: &AuthUser, req: CreateEventRequest) -> AppResult<Value> {
    let foc_id = auth.require_foc()?;
    let title = non_empty("title", &req.title)?;
    let location = non_empty("location", &req.location)?;
    let start_time = non_empty("start_time", &req.start_time)?;
    let end_time = non_empty("end_time", &req.end_time)?;

    let manual_stats = req.manual_stats.unwrap_or_default();
    check_stats(&manual_stats)?;
    if let Some(sermon_id) = req.sermon_id {
        check_sermon(state, sermon_id, foc_id).await?;
    }

    // The server, not the client, decides who owns the event.
    let owner = scope::ownership(req.scope, auth)?;

    // QR is generated here, in the same INSERT (one round trip, no half-created events).
    let id = Uuid::new_v4();
    let now = Utc::now();
    let qr_code = qr::event_qr_data_url(id, now)?;

    let event = NewEvent {
        id,
        title,
        date: req.date,
        start_time,
        end_time,
        location,
        description: req.description,
        scope: owner.scope,
        church_id: owner.church_id,
        province_id: owner.province_id,
        foc_id: owner.foc_id,
        created_by_foc_id: owner.created_by_foc_id,
        is_recurring: req.is_recurring,
        sermon_id: req.sermon_id,
        manual_stats: serde_json::to_value(&manual_stats).map_err(|e| AppError::Internal(e.to_string()))?,
        qr_code,
        qr_generated_at: now,
    };
    Ok(event_repository::insert(&state.db, &event).await?)
}

pub async fn update(state: &SharedState, auth: &AuthUser, id: Uuid, mut req: UpdateEventRequest) -> AppResult<Value> {
    let foc_id = auth.require_foc()?;

    req.title = req.title.map(|v| non_empty("title", &v)).transpose()?;
    req.location = req.location.map(|v| non_empty("location", &v)).transpose()?;
    req.start_time = req.start_time.map(|v| non_empty("start_time", &v)).transpose()?;
    req.end_time = req.end_time.map(|v| non_empty("end_time", &v)).transpose()?;
    if let Some(stats) = &req.manual_stats {
        if !stats.is_valid() {
            return Err(AppError::Validation("manual_stats values cannot be negative.".into()));
        }
    }
    if let Some(Some(sermon_id)) = req.sermon_id {
        check_sermon(state, sermon_id, foc_id).await?;
    }

    match event_repository::update(&state.db, id, auth, &req).await? {
        Some(event) => Ok(event),
        None => Err(not_manageable(state, auth, id).await?),
    }
}

/// A change matched no row. Tell the caller WHY: the event is visible to them but owned by
/// another FOC (403), or it does not exist / is not theirs to even see (404).
async fn not_manageable(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<AppError> {
    Ok(match event_repository::find_visible(&state.db, id, auth).await? {
        Some(_) => AuthError::Forbidden.into(),
        None => EventError::NotFound.into(),
    })
}

/// Replaces React `regenerateEventQRCode`.
pub async fn regenerate_qr(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Value> {
    auth.require_foc()?;
    let now = Utc::now();
    let qr_code = qr::event_qr_data_url(id, now)?;
    match event_repository::set_qr(&state.db, id, auth, &qr_code, now).await? {
        Some(event) => Ok(event),
        None => Err(not_manageable(state, auth, id).await?),
    }
}

pub async fn delete(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<()> {
    auth.require_foc()?;
    match event_repository::delete(&state.db, id, auth).await {
        Ok(0) => Err(not_manageable(state, auth, id).await?),
        Ok(_) => Ok(()),
        // 23503 = foreign key violation (income/serving rows still point at this event)
        Err(e) if e.as_database_error().and_then(|d| d.code()).as_deref() == Some("23503") => {
            Err(EventError::InUse.into())
        }
        Err(e) => Err(e.into()),
    }
}

// ── RSVP & attendance (computed here instead of in React) ──

fn json_map(event: &Value, key: &str) -> HashMap<Uuid, String> {
    event
        .get(key)
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| Some((k.parse::<Uuid>().ok()?, v.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

pub async fn rsvp_stats(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<RsvpStats> {
    let foc_id = auth.require_foc()?;
    let event = get(state, auth, id).await?;
    let rsvps = json_map(&event, "rsvp");

    let ids: Vec<Uuid> = rsvps.keys().copied().collect();
    let names = member_repository::names_by_ids(&state.db, &ids, foc_id).await?;
    let by_id: HashMap<Uuid, MemberName> = names.into_iter().map(|n| (n.id, n)).collect();

    let category = |status: &str| {
        let matching: Vec<&Uuid> = rsvps.iter().filter(|(_, s)| s.as_str() == status).map(|(id, _)| id).collect();
        RsvpCategory {
            count: matching.len(),
            profiles: matching.iter().filter_map(|id| by_id.get(*id).cloned()).collect(),
        }
    };

    let (going, maybe, not_going) = (category("going"), category("maybe"), category("notGoing"));
    let total = rsvps.len();
    let percentage = if total > 0 { going.count as f64 / total as f64 * 100.0 } else { 0.0 };
    Ok(RsvpStats { going, maybe, not_going, total, percentage })
}

pub async fn attendance(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<AttendanceSummary> {
    let foc_id = auth.require_foc()?;
    let event = get(state, auth, id).await?;
    let present = json_map(&event, "attendance");
    let expected = json_map(&event, "rsvp").values().filter(|s| s.as_str() == "going").count();

    let ids: Vec<Uuid> = present.keys().copied().collect();
    let names = member_repository::names_by_ids(&state.db, &ids, foc_id).await?;

    let mut attendees: Vec<Attendee> = names
        .into_iter()
        .filter_map(|n| present.get(&n.id).cloned().map(|ts| Attendee { profile: n, timestamp: ts }))
        .collect();
    attendees.sort_by(|a, b| b.timestamp.cmp(&a.timestamp)); // newest first

    let total = present.len();
    let attendance_rate = if expected > 0 { total as f64 / expected as f64 * 100.0 } else { 0.0 };
    Ok(AttendanceSummary { attendees, total, expected_from_rsvp: expected, attendance_rate })
}

pub async fn mark_attendance(
    state: &SharedState,
    auth: &AuthUser,
    id: Uuid,
    req: MarkAttendanceRequest,
) -> AppResult<Value> {
    let foc_id = auth.require_foc()?;
    // Only members of the caller's own FOC can be checked in by them.
    member_repository::find_in_foc(&state.db, req.member_id, foc_id)
        .await?
        .ok_or(MemberError::NotFound)?;
    event_repository::mark_attendance(&state.db, id, req.member_id, auth)
        .await?
        .ok_or_else(|| EventError::NotFound.into())
}
