//! Reports: every number is for a chosen period and is compared with the period just before it.

use chrono::{Datelike, Duration, NaiveDate, Utc};
use serde_json::{Value, json};

use crate::{errors::{AppError, AppResult}, middleware::authentication::AuthUser, repositories::report_repository as repo, state::SharedState};

pub struct Period { pub from: NaiveDate, pub to: NaiveDate, pub pfrom: NaiveDate, pub pto: NaiveDate }

/// Defaults to the current month so far. The comparison period is the same length, ending the day before.
pub fn period(from: Option<NaiveDate>, to: Option<NaiveDate>) -> AppResult<Period> {
    let today = Utc::now().date_naive();
    let to = to.unwrap_or(today);
    let from = from.unwrap_or_else(|| NaiveDate::from_ymd_opt(to.year(), to.month(), 1).unwrap_or(to));
    if from > to { return Err(AppError::Validation("The start date cannot be after the end date.".into())); }
    let days = (to - from).num_days() + 1;
    if days > 366 * 3 { return Err(AppError::Validation("Choose a period of 3 years or less.".into())); }
    let pto = from - Duration::days(1);
    Ok(Period { from, to, pfrom: pto - Duration::days(days - 1), pto })
}

fn head(p: &Period) -> Value {
    json!({ "from": p.from, "to": p.to, "previous_from": p.pfrom, "previous_to": p.pto, "days": (p.to - p.from).num_days() + 1 })
}

pub async fn overview(state: &SharedState, auth: &AuthUser, from: Option<NaiveDate>, to: Option<NaiveDate>) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let p = period(from, to)?;
    let db = &state.db;
    let members = repo::members(db, foc, p.from, p.to, p.pfrom, p.pto).await?;
    let attendance = json!({
        "current": repo::attendance_summary(db, foc, p.from, p.to).await?,
        "previous": repo::attendance_summary(db, foc, p.pfrom, p.pto).await?,
    });
    // Money only for people who may see it.
    let giving = if auth.can("giving.view") { Some(repo::money(db, "incomes", foc, p.from, p.to, p.pfrom).await?) } else { None };
    let expenses = if auth.can("expenses.view") { Some(repo::money(db, "expenses", foc, p.from, p.to, p.pfrom).await?) } else { None };
    let visitors = if auth.can("visitors.view") { Some(repo::visitors(db, foc, p.from, p.to, p.pfrom, p.pto).await?) } else { None };
    Ok(json!({ "period": head(&p), "members": members, "attendance": attendance, "giving": giving, "expenses": expenses, "visitors": visitors }))
}

pub async fn attendance(state: &SharedState, auth: &AuthUser, from: Option<NaiveDate>, to: Option<NaiveDate>) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let p = period(from, to)?;
    let db = &state.db;
    Ok(json!({
        "period": head(&p),
        "summary": repo::attendance_summary(db, foc, p.from, p.to).await?,
        "previous": repo::attendance_summary(db, foc, p.pfrom, p.pto).await?,
        "services": repo::services(db, foc, p.from, p.to).await?,
        "regularity": repo::regularity(db, foc, p.from, p.to).await?,
        "stopped": repo::stopped(db, foc, p.from, p.to, p.pfrom, p.pto).await?,
    }))
}

pub async fn giving(state: &SharedState, auth: &AuthUser, from: Option<NaiveDate>, to: Option<NaiveDate>) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    if !auth.can("giving.view") { return Err(crate::errors::domain::auth::AuthError::Forbidden.into()); }
    let p = period(from, to)?;
    let db = &state.db;
    let spending = auth.can("expenses.view");
    Ok(json!({
        "period": head(&p),
        "totals": repo::giving_totals(db, "incomes", foc, p.from, p.to, p.pfrom).await?,
        "by_category": repo::split(db, "incomes", "category", foc, p.from, p.to, p.pfrom).await?,
        "by_method": repo::split(db, "incomes", "method", foc, p.from, p.to, p.pfrom).await?,
        "monthly": repo::monthly(db, "incomes", foc, p.from, p.to).await?,
        "givers": repo::giver_movement(db, foc, p.from, p.to, p.pfrom).await?,
        "share": repo::giving_share(db, foc, p.from, p.to).await?,
        "top_givers": repo::top_givers(db, foc, p.from, p.to).await?,
        "pledges": repo::pledges(db, foc).await?,
        "spending": if spending { Some(repo::giving_totals(db, "expenses", foc, p.from, p.to, p.pfrom).await?) } else { None },
        "spending_by_category": if spending { Some(repo::split(db, "expenses", "category", foc, p.from, p.to, p.pfrom).await?) } else { None },
    }))
}
