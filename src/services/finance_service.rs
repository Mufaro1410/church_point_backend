use chrono::Utc;
use rust_decimal::Decimal;
use serde_json::{Value, json};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    errors::{AppError, AppResult, FinanceError},
    middleware::authentication::AuthUser,
    models::{
        common::resolve_range,
        finance::{
            EntriesQuery, EntryChanges, Kind, NewCategory, NewCurrency, NewEntry, NewMethod, RangeQuery,
            SelectCurrency, SelectItem, ValidChanges, ValidEntry, VoidRequest,
        },
    },
    repositories::{event_repository, finance_repository as repo},
    state::SharedState,
    utils::validation::{optional_text, required_text},
};

fn bad(msg: &str) -> AppError {
    AppError::Validation(msg.to_string())
}

fn check_amount(a: Decimal) -> AppResult<Decimal> {
    if a <= Decimal::ZERO {
        return Err(bad("amount must be greater than zero."));
    }
    if a.normalize().scale() > 2 {
        return Err(bad("amount can have at most 2 decimal places."));
    }
    if a >= Decimal::from(1_000_000_000_000i64) {
        return Err(bad("amount is too large."));
    }
    Ok(a)
}

fn check_rate(r: Decimal) -> AppResult<Decimal> {
    if r <= Decimal::ZERO || r >= Decimal::from(1_000_000_000_000i64) || r.normalize().scale() > 6 {
        return Err(bad("rate must be greater than zero, with at most 6 decimal places."));
    }
    Ok(r)
}

fn check_reason(reason: &str) -> AppResult<String> {
    let r = required_text("reason", reason, 300)?;
    if r.chars().count() < 3 {
        return Err(bad("reason must be at least 3 characters."));
    }
    Ok(r)
}

/// The rate to snapshot: USD is always 1; any other currency needs a rate set by this FOC.
async fn rate_for(conn: &mut PgConnection, foc: Uuid, currency: Uuid) -> AppResult<Decimal> {
    let (rate, name) = repo::currency_for_foc(conn, foc, currency).await?.ok_or(FinanceError::CurrencyNotEnabled)?;
    if name.eq_ignore_ascii_case("USD") {
        return Ok(Decimal::ONE);
    }
    rate.ok_or_else(|| FinanceError::MissingRate.into())
}

async fn check_category(conn: &mut PgConnection, foc: Uuid, category: Uuid, kind: Kind) -> AppResult<()> {
    match repo::category_type_for_foc(conn, foc, category).await? {
        None => Err(FinanceError::CategoryNotEnabled.into()),
        Some(t) if t != kind.name() => Err(FinanceError::WrongCategoryType.into()),
        Some(_) => Ok(()),
    }
}

async fn check_method(conn: &mut PgConnection, foc: Uuid, method: Uuid) -> AppResult<()> {
    if repo::method_enabled(conn, foc, method).await? { Ok(()) } else { Err(FinanceError::MethodNotEnabled.into()) }
}

async fn check_member(conn: &mut PgConnection, foc: Uuid, member: Uuid) -> AppResult<()> {
    if repo::member_in_foc(conn, member, foc).await? { Ok(()) } else { Err(bad("member_id does not match a member of your FOC.")) }
}

async fn check_ministry(conn: &mut PgConnection, foc: Uuid, ministry: Uuid) -> AppResult<()> {
    if repo::ministry_enabled(conn, foc, ministry).await? { Ok(()) } else { Err(bad("ministry_id is not one of your FOC's ministries.")) }
}

// ───────────────────────── ledger entries (incomes and expenses) ─────────────────────────

pub async fn create(state: &SharedState, auth: &AuthUser, kind: Kind, req: NewEntry) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let amount = check_amount(req.amount)?;
    let description = optional_text("description", req.description, 500)?;
    let date = req.date.unwrap_or_else(|| Utc::now().date_naive());

    let supplier = match kind {
        Kind::Income => {
            if req.supplier.is_some() || req.ministry_id.is_some() {
                return Err(bad("supplier and ministry_id are only for expenses."));
            }
            None
        }
        Kind::Expense => {
            if req.member_id.is_some() || req.event_id.is_some() {
                return Err(bad("member_id and event_id are only for income."));
            }
            Some(required_text("supplier", req.supplier.as_deref().unwrap_or(""), 150)?)
        }
    };
    if let Some(event) = req.event_id {
        event_repository::find_visible(&state.db, event, auth).await?
            .ok_or_else(|| bad("event_id does not match an event you can see."))?;
    }

    let mut tx = state.db.begin().await?;
    let rate = rate_for(&mut tx, foc, req.currency_id).await?;
    check_category(&mut tx, foc, req.category_id, kind).await?;
    check_method(&mut tx, foc, req.method_id).await?;
    if let Some(m) = req.member_id {
        check_member(&mut tx, foc, m).await?;
    }
    if let Some(m) = req.ministry_id {
        check_ministry(&mut tx, foc, m).await?;
    }

    let entry = ValidEntry {
        amount, currency_id: req.currency_id, category_id: req.category_id, method_id: req.method_id,
        description, date, rate, member_id: req.member_id, event_id: req.event_id, supplier, ministry_id: req.ministry_id,
    };
    let id = repo::insert_entry(&mut tx, kind, foc, auth.member_id, &entry).await?;
    let after = repo::find_entry(&mut tx, kind, id, foc).await?.ok_or(FinanceError::EntryNotFound)?;
    repo::insert_audit(&mut tx, foc, kind, id, "create", auth.member_id, None, None, Some(&after)).await?;
    tx.commit().await?;
    Ok(after)
}

pub async fn get(state: &SharedState, auth: &AuthUser, kind: Kind, id: Uuid) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let mut conn = state.db.acquire().await?;
    repo::find_entry(&mut conn, kind, id, foc).await?.ok_or_else(|| FinanceError::EntryNotFound.into())
}

pub async fn list(state: &SharedState, auth: &AuthUser, kind: Kind, q: EntriesQuery) -> AppResult<Vec<Value>> {
    let foc = auth.require_foc()?;
    let (start, end) = resolve_range(q.start, q.end);
    let filter = repo::EntryFilter {
        start, end, category_id: q.category_id, member_id: q.member_id, event_id: q.event_id,
        include_voided: q.include_voided, limit: q.limit.unwrap_or(200).clamp(1, 1000),
    };
    Ok(repo::list_entries(&state.db, kind, foc, &filter).await?)
}

/// Every change needs a reason and is audited in the SAME transaction as the change itself.
pub async fn update(state: &SharedState, auth: &AuthUser, kind: Kind, id: Uuid, req: EntryChanges) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let reason = check_reason(&req.reason)?;

    let mut ch = ValidChanges {
        amount: req.amount.map(check_amount).transpose()?,
        category_id: req.category_id,
        method_id: req.method_id,
        description: req.description.map(|d| optional_text("description", d, 500)).transpose()?,
        date: req.date,
        ..Default::default()
    };
    match kind {
        Kind::Income => {
            if req.supplier.is_some() || req.ministry_id.is_some() {
                return Err(bad("supplier and ministry_id are only for expenses."));
            }
            ch.member_id = req.member_id;
        }
        Kind::Expense => {
            if req.member_id.is_some() {
                return Err(bad("member_id is only for income."));
            }
            ch.supplier = req.supplier.map(|s| required_text("supplier", &s, 150)).transpose()?;
            ch.ministry_id = req.ministry_id;
        }
    }

    let mut tx = state.db.begin().await?;
    let current = repo::lock_entry(&mut tx, kind, id, foc).await?.ok_or(FinanceError::EntryNotFound)?;
    if current.voided {
        return Err(FinanceError::EntryVoided.into());
    }
    let before = repo::find_entry(&mut tx, kind, id, foc).await?.ok_or(FinanceError::EntryNotFound)?;

    // a changed currency re-snapshots the rate; an unchanged currency keeps the rate it was recorded with
    if let Some(c) = req.currency_id {
        if c != current.currency_id {
            ch.currency = Some((c, rate_for(&mut tx, foc, c).await?));
        }
    }
    if let Some(c) = ch.category_id {
        check_category(&mut tx, foc, c, kind).await?;
    }
    if let Some(m) = ch.method_id {
        check_method(&mut tx, foc, m).await?;
    }
    if let Some(Some(m)) = ch.member_id {
        check_member(&mut tx, foc, m).await?;
    }
    if let Some(Some(m)) = ch.ministry_id {
        check_ministry(&mut tx, foc, m).await?;
    }

    repo::update_entry(&mut tx, kind, id, foc, &ch).await?;
    let after = repo::find_entry(&mut tx, kind, id, foc).await?.ok_or(FinanceError::EntryNotFound)?;
    repo::insert_audit(&mut tx, foc, kind, id, "update", auth.member_id, Some(&reason), Some(&before), Some(&after)).await?;
    tx.commit().await?;
    Ok(after)
}

pub async fn void(state: &SharedState, auth: &AuthUser, kind: Kind, id: Uuid, req: VoidRequest) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let reason = check_reason(&req.reason)?;

    let mut tx = state.db.begin().await?;
    let current = repo::lock_entry(&mut tx, kind, id, foc).await?.ok_or(FinanceError::EntryNotFound)?;
    if current.voided {
        return Err(FinanceError::EntryVoided.into());
    }
    let before = repo::find_entry(&mut tx, kind, id, foc).await?.ok_or(FinanceError::EntryNotFound)?;
    repo::void_entry(&mut tx, kind, id, foc, auth.member_id, &reason).await?;
    let after = repo::find_entry(&mut tx, kind, id, foc).await?.ok_or(FinanceError::EntryNotFound)?;
    repo::insert_audit(&mut tx, foc, kind, id, "void", auth.member_id, Some(&reason), Some(&before), Some(&after)).await?;
    tx.commit().await?;
    Ok(after)
}

pub async fn history(state: &SharedState, auth: &AuthUser, record: Uuid) -> AppResult<Vec<Value>> {
    let foc = auth.require_foc()?;
    Ok(repo::history(&state.db, foc, record).await?)
}

pub async fn summary(state: &SharedState, auth: &AuthUser, q: RangeQuery) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let (start, end) = resolve_range(q.start, q.end);
    let income = repo::summary(&state.db, Kind::Income, foc, start, end).await?;
    let expense = repo::summary(&state.db, Kind::Expense, foc, start, end).await?;
    let total = |v: &Value| v["total_usd"].as_str().and_then(|s| s.parse::<Decimal>().ok()).unwrap_or(Decimal::ZERO);
    let net = total(&income) - total(&expense);
    Ok(json!({ "start": start, "end": end, "base_currency": "USD", "income": income, "expense": expense, "net_usd": net.to_string() }))
}

// ───────────────────────── catalogue set-up (what this FOC accepts and spends in) ─────────────────────────

pub async fn setup(state: &SharedState, auth: &AuthUser) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    Ok(repo::setup(&state.db, foc).await?)
}

pub async fn set_currency(state: &SharedState, auth: &AuthUser, id: Uuid, req: SelectCurrency) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let name = repo::currency_name(&state.db, id).await?.ok_or(FinanceError::CatalogueItemNotFound)?;
    if !req.selected {
        repo::deselect(&state.db, "foc_currencies", "currency_id", foc, id).await?;
    } else {
        let rate = if name.eq_ignore_ascii_case("USD") {
            if req.rate.is_some_and(|r| r != Decimal::ONE) {
                return Err(bad("USD is the base currency: its rate is always 1."));
            }
            Some(Decimal::ONE)
        } else {
            req.rate.map(check_rate).transpose()?
        };
        repo::select_currency(&state.db, foc, id, rate).await?;
    }
    setup(state, auth).await
}

pub async fn add_currency(state: &SharedState, auth: &AuthUser, req: NewCurrency) -> AppResult<Value> {
    auth.require_foc()?;
    let name = req.name.trim().to_uppercase();
    if !(3..=5).contains(&name.len()) || !name.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(bad("currency name must be a 3 to 5 letter code such as USD or ZWL."));
    }
    let symbol = optional_text("symbol", req.symbol, 8)?.unwrap_or_default();
    let id = repo::create_currency(&state.db, &name, &symbol).await?;
    set_currency(state, auth, id, SelectCurrency { selected: true, rate: req.rate }).await
}

pub async fn set_category(state: &SharedState, auth: &AuthUser, id: Uuid, req: SelectItem) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    if req.selected {
        if !repo::select_category(&state.db, foc, id).await? {
            return Err(FinanceError::CatalogueItemNotFound.into());
        }
    } else {
        repo::deselect(&state.db, "foc_categories", "category_id", foc, id).await?;
    }
    setup(state, auth).await
}

pub async fn add_category(state: &SharedState, auth: &AuthUser, req: NewCategory) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let name = required_text("name", &req.name, 60)?;
    if req.kind != "income" && req.kind != "expense" {
        return Err(bad("type must be income or expense."));
    }
    let description = optional_text("description", req.description, 200)?;
    let (id, existing_type) = repo::create_category(&state.db, &name, &req.kind, description.as_deref()).await?;
    if existing_type != req.kind {
        return Err(FinanceError::CategoryTypeConflict.into());
    }
    repo::select_category(&state.db, foc, id).await?;
    setup(state, auth).await
}

pub async fn set_method(state: &SharedState, auth: &AuthUser, id: Uuid, req: SelectItem) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    if req.selected {
        if !repo::select_method(&state.db, foc, id).await? {
            return Err(FinanceError::CatalogueItemNotFound.into());
        }
    } else {
        repo::deselect(&state.db, "foc_methods", "method_id", foc, id).await?;
    }
    setup(state, auth).await
}

pub async fn add_method(state: &SharedState, auth: &AuthUser, req: NewMethod) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let name = required_text("name", &req.name, 60)?;
    let description = optional_text("description", req.description, 200)?;
    let id = repo::create_method(&state.db, &name, description.as_deref()).await?;
    repo::select_method(&state.db, foc, id).await?;
    setup(state, auth).await
}

pub async fn usage(state: &SharedState, auth: &AuthUser, column: &'static str, id: Uuid) -> AppResult<Value> {
    let n = crate::repositories::finance_repository::usage_count(&state.db, auth.require_foc()?, column, id).await?;
    Ok(serde_json::json!({ "count": n }))
}
