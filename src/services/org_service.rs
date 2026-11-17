use serde_json::Value;
use uuid::Uuid;

use crate::{
    errors::{AppError, AppResult, OrgError},
    middleware::authentication::AuthUser,
    models::org::{CellGroupRequest, MeetingRequest},
    repositories::org_repository as repo,
    state::SharedState,
};

fn trimmed(field: &str, v: &mut Option<String>) -> AppResult<()> {
    if let Some(s) = v {
        *s = s.trim().to_string();
        if s.is_empty() {
            return Err(AppError::Validation(format!("{field} cannot be empty.")));
        }
    }
    Ok(())
}

fn time_ok(field: &str, v: &mut Option<String>) -> AppResult<()> {
    if let Some(s) = v {
        let t = chrono::NaiveTime::parse_from_str(s, "%H:%M")
            .or_else(|_| chrono::NaiveTime::parse_from_str(s, "%H:%M:%S"))
            .map_err(|_| AppError::Validation(format!("{field} must look like 14:30.")))?;
        *s = t.format("%H:%M").to_string();
    }
    Ok(())
}

async fn check_group(state: &SharedState, foc: Uuid, r: &mut CellGroupRequest) -> AppResult<()> {
    trimmed("name", &mut r.name)?;
    trimmed("meeting_day", &mut r.meeting_day)?;
    time_ok("start_time", &mut r.start_time)?;
    time_ok("end_time", &mut r.end_time)?;
    if let Some(l) = r.leader_id {
        if !repo::leader_in_foc(&state.db, l, foc).await? {
            return Err(OrgError::LeaderNotInFoc.into());
        }
    }
    Ok(())
}

pub async fn list_groups(state: &SharedState, auth: &AuthUser) -> AppResult<Vec<Value>> {
    Ok(repo::list_groups(&state.db, auth.require_foc()?).await?)
}

pub async fn get_group(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Value> {
    repo::get_group(&state.db, id, auth.require_foc()?).await?.ok_or_else(|| OrgError::CellGroupNotFound.into())
}

pub async fn create_group(state: &SharedState, auth: &AuthUser, mut r: CellGroupRequest) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    if r.name.is_none() {
        return Err(AppError::Validation("name is required.".into()));
    }
    check_group(state, foc, &mut r).await?;
    Ok(repo::insert_group(&state.db, foc, &r).await?)
}

pub async fn update_group(state: &SharedState, auth: &AuthUser, id: Uuid, mut r: CellGroupRequest) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    check_group(state, foc, &mut r).await?;
    repo::update_group(&state.db, id, foc, &r).await?.ok_or_else(|| OrgError::CellGroupNotFound.into())
}

pub async fn delete_group(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<()> {
    match repo::delete_group(&state.db, id, auth.require_foc()?).await? {
        0 => Err(OrgError::CellGroupNotFound.into()),
        _ => Ok(()),
    }
}

pub async fn list_all_meetings(state: &SharedState, auth: &AuthUser) -> AppResult<Vec<Value>> {
    Ok(repo::list_all_meetings(&state.db, auth.require_foc()?).await?)
}

pub async fn list_meetings(state: &SharedState, auth: &AuthUser, group: Uuid) -> AppResult<Vec<Value>> {
    let foc = auth.require_foc()?;
    if repo::get_group(&state.db, group, foc).await?.is_none() {
        return Err(OrgError::CellGroupNotFound.into());
    }
    Ok(repo::list_meetings(&state.db, group, foc).await?)
}

pub async fn create_meeting(state: &SharedState, auth: &AuthUser, group: Uuid, mut r: MeetingRequest) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let date = r.date.as_deref().ok_or_else(|| AppError::Validation("date is required.".into()))?;
    if chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
        return Err(AppError::Validation("date must look like 2026-11-30.".into()));
    }
    for (name, v) in [("attendance", &r.attendance), ("offering", &r.offering)] {
        if v.as_ref().is_some_and(|x| !x.is_array()) {
            return Err(AppError::Validation(format!("{name} must be a list.")));
        }
    }
    trimmed("title", &mut r.title)?;
    repo::insert_meeting(&state.db, group, foc, auth.member_id, &r)
        .await?
        .ok_or_else(|| OrgError::CellGroupNotFound.into())
}

pub async fn delete_meeting(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<()> {
    match repo::delete_meeting(&state.db, id, auth.require_foc()?).await? {
        0 => Err(OrgError::MeetingNotFound.into()),
        _ => Ok(()),
    }
}

pub async fn update_meeting(state: &SharedState, auth: &AuthUser, id: Uuid, mut r: MeetingRequest) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    if let Some(d) = r.date.as_deref() {
        if chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_err() {
            return Err(AppError::Validation("date must look like 2026-11-30.".into()));
        }
    }
    for (name, v) in [("attendance", &r.attendance), ("offering", &r.offering)] {
        if v.as_ref().is_some_and(|x| !x.is_array()) {
            return Err(AppError::Validation(format!("{name} must be a list.")));
        }
    }
    trimmed("title", &mut r.title)?;
    repo::update_meeting(&state.db, id, foc, &r).await?.ok_or_else(|| OrgError::MeetingNotFound.into())
}

pub async fn dashboard(state: &SharedState, auth: &AuthUser) -> AppResult<Value> {
    Ok(repo::dashboard(&state.db, auth.require_foc()?).await?)
}
