//! Giving statements: what a member gave in a period, for them to keep (tax, records, thanks).

use chrono::{Datelike, NaiveDate, Utc};
use serde_json::Value;
use uuid::Uuid;

use crate::{errors::{AppError, AppResult}, middleware::authentication::AuthUser, repositories::statement_repository as repo, state::SharedState};

/// Defaults to this year so far; the period is at most 5 years.
fn period(from: Option<NaiveDate>, to: Option<NaiveDate>) -> AppResult<(NaiveDate, NaiveDate)> {
    let today = Utc::now().date_naive();
    let to = to.unwrap_or(today);
    let from = from.unwrap_or_else(|| NaiveDate::from_ymd_opt(to.year(), 1, 1).unwrap_or(to));
    if from > to { return Err(AppError::Validation("The start date cannot be after the end date.".into())); }
    if (to - from).num_days() > 366 * 5 { return Err(AppError::Validation("Choose a period of 5 years or less.".into())); }
    Ok((from, to))
}

pub async fn summary(state: &SharedState, auth: &AuthUser, from: Option<NaiveDate>, to: Option<NaiveDate>) -> AppResult<Value> {
    let (f, t) = period(from, to)?;
    let rows = repo::summary(&state.db, auth.require_foc()?, f, t).await?;
    Ok(serde_json::json!({ "from": f, "to": t, "members": rows }))
}

pub async fn statement(state: &SharedState, auth: &AuthUser, member: Uuid, from: Option<NaiveDate>, to: Option<NaiveDate>) -> AppResult<Value> {
    let (f, t) = period(from, to)?;
    repo::statement(&state.db, auth.require_foc()?, member, f, t).await?
        .ok_or_else(|| AppError::Validation("Member not found.".into()))
}
