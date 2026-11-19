use chrono::Utc;
use serde_json::Value;
use uuid::Uuid;

use super::scope;
use crate::{
    errors::{AnnouncementError, AppError, AppResult, AuthError},
    middleware::authentication::AuthUser,
    models::announcement::{
        AnnouncementsQuery, CreateAnnouncementRequest, NewAnnouncement, Priority,
        UpdateAnnouncementRequest,
    },
    repositories::announcement_repository,
    state::SharedState,
    utils::validation::{optional_text, required_text},
};

pub async fn list(state: &SharedState, auth: &AuthUser, q: AnnouncementsQuery) -> AppResult<Vec<Value>> {
    auth.require_foc()?;
    Ok(announcement_repository::list_visible(&state.db, auth, q.active).await?)
}

pub async fn get(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Value> {
    auth.require_foc()?;
    announcement_repository::find_visible(&state.db, id, auth)
        .await?
        .ok_or_else(|| AnnouncementError::NotFound.into())
}

fn check_dates(start: chrono::NaiveDate, end: Option<chrono::NaiveDate>) -> AppResult<()> {
    if end.is_some_and(|e| e < start) {
        return Err(AppError::Validation("end_date cannot be before start_date.".into()));
    }
    Ok(())
}

pub async fn create(state: &SharedState, auth: &AuthUser, r: CreateAnnouncementRequest) -> AppResult<Value> {
    // Server decides ownership (same rules as events).
    let owner = scope::ownership(r.scope, auth)?;
    let start_date = r.start_date.unwrap_or_else(|| Utc::now().date_naive());
    check_dates(start_date, r.end_date)?;

    let announcement = NewAnnouncement {
        title: required_text("title", &r.title, 200)?,
        content: required_text("content", &r.content, 5000)?,
        scope: owner.scope,
        church_id: owner.church_id,
        province_id: owner.province_id,
        foc_id: owner.foc_id,
        created_by_foc_id: owner.created_by_foc_id,
        // Keeps the existing convention: created_by holds the auth user id.
        created_by: auth.user_id,
        priority: r.priority.unwrap_or(Priority::Medium).as_str(),
        category: optional_text("category", r.category, 100)?,
        start_date,
        end_date: r.end_date,
        is_active: r.is_active.unwrap_or(true),
    };
    let row = announcement_repository::insert(&state.db, &announcement).await?;
    crate::services::push_service::announcement_created(state, &row);
    Ok(row)
}

/// A change matched no row: visible-but-not-yours is 403, otherwise 404.
async fn why_failed(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<AppError> {
    Ok(match announcement_repository::find_visible(&state.db, id, auth).await? {
        Some(_) => AuthError::Forbidden.into(),
        None => AnnouncementError::NotFound.into(),
    })
}

pub async fn update(state: &SharedState, auth: &AuthUser, id: Uuid, mut r: UpdateAnnouncementRequest) -> AppResult<Value> {
    auth.require_foc()?;
    r.title = r.title.map(|v| required_text("title", &v, 200)).transpose()?;
    r.content = r.content.map(|v| required_text("content", &v, 5000)).transpose()?;
    r.category = r.category.map(|v| optional_text("category", v, 100)).transpose()?;

    // end_date >= start_date must hold for the RESULTING row, so merge with what's stored.
    if r.start_date.is_some() || r.end_date.is_some() {
        if let Some((cur_start, cur_end)) = announcement_repository::dates(&state.db, id, auth).await? {
            let start = r.start_date.unwrap_or(cur_start);
            let end = match r.end_date {
                Some(v) => v,
                None => cur_end,
            };
            check_dates(start, end)?;
        }
    }

    match announcement_repository::update(&state.db, id, auth, &r).await? {
        Some(a) => Ok(a),
        None => Err(why_failed(state, auth, id).await?),
    }
}

pub async fn delete(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<()> {
    auth.require_foc()?;
    match announcement_repository::delete(&state.db, id, auth).await? {
        0 => Err(why_failed(state, auth, id).await?),
        _ => Ok(()),
    }
}
