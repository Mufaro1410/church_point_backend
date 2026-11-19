use chrono::Utc;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    errors::{AppError, AppResult, AuthError, PrayerError},
    middleware::authentication::AuthUser,
    models::{
        common::{DateRangeQuery, resolve_range},
        prayer::{CreatePrayerRequest, UpdatePrayerRequest},
    },
    repositories::prayer_repository,
    state::SharedState,
    utils::validation::required_text,
};

const MAX_TITLE: usize = 150;
const MAX_CONTENT: usize = 2000;

pub async fn list(state: &SharedState, auth: &AuthUser, q: DateRangeQuery) -> AppResult<Vec<Value>> {
    let foc_id = auth.require_foc()?;
    let (start, end) = resolve_range(q.start, q.end);
    Ok(prayer_repository::list(&state.db, foc_id, auth.member_id, start, end).await?)
}

pub async fn get(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Value> {
    let foc_id = auth.require_foc()?;
    prayer_repository::find(&state.db, id, foc_id, auth.member_id)
        .await?
        .ok_or_else(|| PrayerError::NotFound.into())
}

pub async fn create(state: &SharedState, auth: &AuthUser, r: CreatePrayerRequest) -> AppResult<Value> {
    let foc_id = auth.require_foc()?;
    let title = required_text("title", &r.title, MAX_TITLE)?;
    let content = required_text("content", &r.content, MAX_CONTENT)?;
    let date = r.date.unwrap_or_else(|| Utc::now().date_naive());

    // FIX: the author is the caller's MEMBER id. The React code sent the auth user id,
    // which does not match the member_profiles foreign key on this column.
    let id = prayer_repository::insert(&state.db, foc_id, auth.member_id, &title, &content, r.is_anonymous, date)
        .await?;
    get(state, auth, id).await
}

/// After a failed author-only change: 403 if the prayer exists (it's someone else's), else 404.
async fn why_failed(state: &SharedState, foc_id: Uuid, id: Uuid) -> AppResult<AppError> {
    Ok(if prayer_repository::exists(&state.db, id, foc_id).await? {
        AuthError::Forbidden.into()
    } else {
        PrayerError::NotFound.into()
    })
}

/// Only the author may edit their prayer.
pub async fn update(state: &SharedState, auth: &AuthUser, id: Uuid, mut r: UpdatePrayerRequest) -> AppResult<Value> {
    let foc_id = auth.require_foc()?;
    r.title = r.title.map(|v| required_text("title", &v, MAX_TITLE)).transpose()?;
    r.content = r.content.map(|v| required_text("content", &v, MAX_CONTENT)).transpose()?;

    match prayer_repository::update_own(&state.db, id, foc_id, auth.member_id, &r).await? {
        Some(id) => get(state, auth, id).await,
        None => Err(why_failed(state, foc_id, id).await?),
    }
}

/// The author, or an admin moderating.
pub async fn delete(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<()> {
    let foc_id = auth.require_foc()?;
    match prayer_repository::delete(&state.db, id, foc_id, auth.member_id, auth.can("prayers.delete")).await? {
        0 => Err(why_failed(state, foc_id, id).await?),
        _ => Ok(()),
    }
}
