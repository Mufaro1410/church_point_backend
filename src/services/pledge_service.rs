//! Pledges: promises of giving to a fund. Progress is worked out from the giving ledger.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    errors::{AppError, AppResult},
    middleware::authentication::AuthUser,
    models::finance::Kind,
    repositories::pledge_repository as repo,
    services::finance_service::{check_amount, check_category},
    state::SharedState,
    utils::validation::optional_text,
};

fn bad(m: &str) -> AppError { AppError::Validation(m.into()) }

pub async fn list(state: &SharedState, auth: &AuthUser, member: Option<Uuid>) -> AppResult<Vec<Value>> {
    Ok(repo::list(&state.db, auth.require_foc()?, member).await?)
}

pub async fn get(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Value> {
    repo::get(&state.db, auth.require_foc()?, id).await?.ok_or_else(|| bad("Pledge not found."))
}

pub struct NewPledge {
    pub member_id: Option<Uuid>,
    pub ministry_id: Option<Uuid>,
    pub pledger_name: Option<String>,
    pub category_id: Uuid,
    pub currency_id: Uuid,
    pub amount: Decimal,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub note: Option<String>,
}

pub async fn create(state: &SharedState, auth: &AuthUser, req: NewPledge) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let amount = check_amount(req.amount)?;
    let note = optional_text("note", req.note, 500)?;
    if let (Some(s), Some(e)) = (req.start_date, req.end_date) && e < s { return Err(bad("The end date cannot be before the start date.")); }

    let mut tx = state.db.begin().await?;
    // the currency must be one this church uses (its rate is not needed: a pledge is tracked in its own currency)
    crate::repositories::finance_repository::currency_for_foc(&mut tx, foc, req.currency_id).await?
        .ok_or(crate::errors::FinanceError::CurrencyNotEnabled)?;
    check_category(&mut tx, foc, req.category_id, Kind::Income).await?;
    let other = optional_text("pledger_name", req.pledger_name, 150)?.filter(|n| !n.is_empty());
    if [req.member_id.is_some(), req.ministry_id.is_some(), other.is_some()].iter().filter(|x| **x).count() != 1 {
        return Err(bad("Choose exactly one pledger: a member, a ministry, or a name."));
    }
    if let Some(m) = req.member_id && !crate::repositories::finance_repository::member_in_foc(&mut tx, m, foc).await? {
        return Err(bad("member_id does not match a member of your church."));
    }
    if let Some(m) = req.ministry_id && !crate::repositories::finance_repository::ministry_enabled(&mut tx, foc, m).await? {
        return Err(bad("ministry_id is not one of your church's ministries."));
    }
    let id = repo::insert(&mut tx, foc, auth.member_id, req.member_id, req.ministry_id, other.as_deref(), req.category_id, req.currency_id, amount, req.start_date, req.end_date, note.as_deref()).await?;
    tx.commit().await?;
    get(state, auth, id).await
}

pub async fn update(state: &SharedState, auth: &AuthUser, id: Uuid, amount: Option<Decimal>, end: Option<NaiveDate>, status: Option<String>, note: Option<String>) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let amount = amount.map(check_amount).transpose()?;
    let note = optional_text("note", note, 500)?;
    if let Some(s) = &status && s != "active" && s != "cancelled" { return Err(bad("Unknown status.")); }
    let current = get(state, auth, id).await?;
    if let (Some(e), Some(s)) = (end, current["start_date"].as_str().and_then(|s| s.parse::<NaiveDate>().ok())) && e < s {
        return Err(bad("The end date cannot be before the start date."));
    }
    if !repo::update(&state.db, foc, id, amount, end, status.as_deref(), note.as_deref()).await? { return Err(bad("Pledge not found.")); }
    get(state, auth, id).await
}
