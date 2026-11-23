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

pub async fn members(state: &SharedState, auth: &AuthUser, from: Option<NaiveDate>, to: Option<NaiveDate>) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let p = period(from, to)?;
    let db = &state.db;
    Ok(json!({
        "period": head(&p),
        "now": repo::members(db, foc, p.from, p.to, p.pfrom, p.pto).await?,
        "series": repo::growth_series(db, foc, p.from, p.to).await?,
        "composition": repo::composition(db, foc).await?,
        "newcomers": repo::newcomers(db, foc, p.from, p.to).await?,
        "birthdays": repo::birthdays(db, foc).await?,
    }))
}

pub async fn cells(state: &SharedState, auth: &AuthUser, from: Option<NaiveDate>, to: Option<NaiveDate>) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let p = period(from, to)?;
    let db = &state.db;
    Ok(json!({
        "period": head(&p),
        "cells": repo::cells(db, foc, p.from, p.to, p.pfrom, p.pto).await?,
        "offerings": repo::cell_offerings(db, foc, p.from, p.to).await?,
        "absentees": repo::cell_absentees(db, foc, p.from, p.to).await?,
        "not_in_cell": repo::members(db, foc, p.from, p.to, p.pfrom, p.pto).await?["not_in_cell"],
    }))
}

pub async fn visitors(state: &SharedState, auth: &AuthUser, from: Option<NaiveDate>, to: Option<NaiveDate>) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    if !auth.can("visitors.view") { return Err(crate::errors::domain::auth::AuthError::Forbidden.into()); }
    let p = period(from, to)?;
    let db = &state.db;
    Ok(json!({
        "period": head(&p),
        "funnel": repo::visitor_funnel(db, foc, p.from, p.to, p.pfrom, p.pto).await?,
        "sources": repo::visitor_sources(db, foc, p.from, p.to).await?,
        "waiting": repo::visitors_waiting(db, foc).await?,
        "followups": repo::visitor_followups(db, foc, p.from, p.to).await?,
    }))
}

pub async fn community(state: &SharedState, auth: &AuthUser, from: Option<NaiveDate>, to: Option<NaiveDate>) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let p = period(from, to)?;
    let db = &state.db;
    Ok(json!({
        "period": head(&p),
        "prayers": repo::prayer_stats(db, foc, p.from, p.to, p.pfrom, p.pto).await?,
        "unsupported": repo::unsupported_prayers(db, foc, p.from, p.to).await?,
        "testimonies": repo::testimony_stats(db, foc, p.from, p.to, p.pfrom, p.pto).await?,
        "sermons": repo::sermon_stats(db, foc, p.from, p.to, p.pfrom, p.pto).await?,
        "announcements": repo::announcement_stats(db, foc).await?,
        "participation": repo::participation(db, foc, p.from, p.to).await?,
        "moderation": if auth.can("moderation.view") { Some(repo::reports_summary(db, foc, p.from, p.to).await?) } else { None },
    }))
}

/// Small counts for the Home page, each only when the caller may see the thing counted.
pub async fn attention(state: &SharedState, auth: &AuthUser) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let db = &state.db;
    let today = Utc::now().date_naive();
    let visitors = if auth.can("visitors.view") { Some(repo::visitors_waiting(db, foc).await?.len()) } else { None };
    let stopped = if auth.can("visitors.view") {
        // Seen in the 4 weeks before the last 4 weeks, but not once in the last 4 weeks.
        let (from, pto) = (today - Duration::days(27), today - Duration::days(28));
        Some(repo::stopped(db, foc, from, today, pto - Duration::days(27), pto).await?.len())
    } else { None };
    let overdue = if auth.can("giving.view") {
        Some(repo::pledges(db, foc).await?.iter().map(|r| r["overdue"].as_i64().unwrap_or(0)).sum::<i64>())
    } else { None };
    Ok(json!({ "visitors_waiting": visitors, "stopped_attending": stopped, "overdue_pledges": overdue }))
}
