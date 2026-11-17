//! Mobile-money payments through each church's own Paynow account.
//!
//! The truth about a payment always comes from asking Paynow (poll), never from an incoming message.
//! Recording the income is repeat-safe: the payment row is locked and only gets one income.

use rust_decimal::Decimal;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    clients::paynow::{self, MobileOrder},
    errors::{AppError, AppResult, PaymentError},
    middleware::authentication::AuthUser,
    models::payment::{NewPayment, PaymentSettingsRequest, PaymentsQuery},
    repositories::payment_repository as repo,
    services::finance_service,
    state::SharedState,
    utils::{crypto, validation::{optional_text, required_text}},
};

fn key(state: &SharedState) -> AppResult<&[u8; 32]> {
    state.settings.payments_key.as_ref().ok_or(AppError::NotConfigured("PAYMENTS_ENCRYPTION_KEY"))
}

// ── settings (church admin) ──

pub async fn get_settings(state: &SharedState, auth: &AuthUser) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    Ok(repo::settings_json(&state.db, foc).await?.unwrap_or(serde_json::json!({ "enabled": false, "key_saved": false })))
}

pub async fn put_settings(state: &SharedState, auth: &AuthUser, req: PaymentSettingsRequest) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let id = required_text("integration_id", &req.integration_id, 40)?;
    let email = required_text("merchant_email", &req.merchant_email, 200)?;
    if !email.contains('@') {
        return Err(AppError::Validation("merchant_email must be an email address.".into()));
    }
    let new_key = req.integration_key.as_deref().map(str::trim).filter(|k| !k.is_empty());
    let enc = new_key.map(|k| crypto::encrypt(key(state)?, k)).transpose()?;
    let existing = repo::settings(&state.db, foc).await?;
    if enc.is_none() && existing.as_ref().is_none_or(|s| s.integration_key_enc.is_empty()) {
        return Err(AppError::Validation("integration_key is required the first time.".into()));
    }
    {
        let mut conn = state.db.acquire().await?;
        finance_service::check_category(&mut conn, foc, req.category_id, crate::models::finance::Kind::Income).await?;
        finance_service::check_method(&mut conn, foc, req.method_id).await?;
    }
    repo::upsert_settings(&state.db, foc, &id, enc.as_deref(), &email, req.test_mode, req.category_id, req.method_id, req.enabled).await?;
    get_settings(state, auth).await
}

// ── paying ──

fn clean_phone(method: &str, raw: &str) -> AppResult<String> {
    let mut p: String = raw.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
    if let Some(r) = p.strip_prefix("+263").or_else(|| p.strip_prefix("263")) {
        p = format!("0{r}");
    }
    let ok_prefix = match method {
        "ecocash" => p.starts_with("077") || p.starts_with("078"),
        "onemoney" => p.starts_with("071"),
        _ => false,
    };
    if p.len() != 10 || !p.chars().all(|c| c.is_ascii_digit()) || !ok_prefix {
        return Err(PaymentError::BadPhone.into());
    }
    Ok(p)
}

fn map_status(raw: &str) -> &'static str {
    match raw.trim().to_lowercase().as_str() {
        "paid" | "awaiting delivery" | "delivered" => "paid",
        "created" | "sent" => "pending",
        "cancelled" => "cancelled",
        _ => "failed", // disputed, refunded, anything unknown
    }
}

/// The church's Paynow details with the key decrypted.
async fn creds(state: &SharedState, foc: Uuid) -> AppResult<(repo::SettingsRow, String)> {
    let s = repo::settings(&state.db, foc).await?.ok_or(PaymentError::NotSetUp)?;
    let k = crypto::decrypt(key(state)?, &s.integration_key_enc)?;
    Ok((s, k))
}

pub async fn start(state: &SharedState, auth: &AuthUser, req: NewPayment) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let (settings, key) = creds(state, foc).await?;
    if !settings.enabled || settings.category_id.is_none() || settings.method_id.is_none() {
        return Err(PaymentError::NotSetUp.into());
    }
    let base = state.settings.public_base_url.as_deref().ok_or(AppError::NotConfigured("PUBLIC_BASE_URL"))?;

    let amount = finance_service::check_amount(req.amount)?;
    let method = req.method.trim().to_lowercase();
    let phone = clean_phone(&method, &req.phone)?;
    let purpose = optional_text("purpose", req.purpose, 200)?;
    {
        let mut conn = state.db.acquire().await?;
        finance_service::rate_for(&mut conn, foc, req.currency_id).await?; // currency must be enabled with a rate
    }

    let id = repo::insert(&state.db, foc, auth.member_id, amount, req.currency_id, purpose.as_deref(), &method, &phone).await?;

    // In Paynow's test mode the "customer" email must be the merchant's own login email.
    let member_email = repo::member_email(&state.db, auth.member_id).await?;
    let email = if settings.test_mode { settings.merchant_email.clone() } else { member_email.unwrap_or(settings.merchant_email.clone()) };
    let (reference, amount_text) = (id.to_string(), format!("{amount:.2}"));
    let result_url = format!("{base}/api/v1/webhooks/paynow/{id}");
    let info = purpose.clone().unwrap_or_else(|| "Church giving".into());

    let sent = state.paynow.initiate_mobile(&MobileOrder {
        integration_id: &settings.integration_id, key: &key, reference: &reference, amount: &amount_text, info: &info,
        return_url: base, result_url: &result_url, auth_email: &email, phone: &phone, method: &method,
    }).await;

    match sent {
        Ok(s) => repo::mark_sent(&state.db, id, s.paynow_reference.as_deref(), &s.poll_url, s.instructions.as_deref()).await?,
        Err(e) => {
            repo::mark_failed(&state.db, id, &e.to_string()).await?;
            return Err(e);
        }
    }
    view(state, id).await
}

async fn view(state: &SharedState, id: Uuid) -> AppResult<Value> {
    repo::view(&state.db, id).await?.ok_or_else(|| PaymentError::NotFound.into())
}

/// Admins see every payment of their church; anyone else only their own.
pub async fn get(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let row = repo::find(&state.db, id).await?.filter(|p| p.foc_id == foc).ok_or(PaymentError::NotFound)?;
    let admin = matches!(auth.role.as_deref(), Some("admin" | "super_admin"));
    if !admin && row.member_id != Some(auth.member_id) {
        return Err(PaymentError::NotFound.into());
    }
    refresh(state, &row).await?;
    view(state, id).await
}

pub async fn list(state: &SharedState, auth: &AuthUser, q: PaymentsQuery) -> AppResult<Vec<Value>> {
    let foc = auth.require_foc()?;
    Ok(repo::list(&state.db, foc, q.status.as_deref(), q.limit.unwrap_or(100).clamp(1, 500)).await?)
}

/// Brings a payment up to date: ask Paynow if it is still pending; retry the income if paid but not yet recorded.
async fn refresh(state: &SharedState, p: &repo::PaymentRow) -> AppResult<()> {
    match (p.status.as_str(), p.income_id) {
        ("pending", _) => {
            let Some(url) = p.poll_url.as_deref() else { return Ok(()) };
            let (_, key) = creds(state, p.foc_id).await?;
            let polled = state.paynow.poll(url, &key).await?;
            apply(state, p.id, polled).await
        }
        ("paid", None) => record_income(state, p.id).await,
        _ => Ok(()),
    }
}

async fn apply(state: &SharedState, id: Uuid, polled: paynow::Polled) -> AppResult<()> {
    let new = map_status(&polled.status);
    let mut tx = state.db.begin().await?;
    let Some(row) = repo::lock(&mut tx, id).await? else { return Ok(()) };
    if row.status == "paid" {
        return Ok(()); // already settled; never goes backwards
    }
    let mut status = new;
    let mut error = None;
    if new == "paid" {
        let same = polled.amount.as_deref().and_then(|a| a.parse::<Decimal>().ok()).is_some_and(|a| a == row.amount);
        if !same {
            status = "failed";
            error = Some("Paynow reported a different amount than was requested.");
        }
    }
    repo::set_status(&mut tx, id, status, &polled.status, polled.paynow_reference.as_deref(), error).await?;
    tx.commit().await?;
    if status == "paid" {
        record_income(state, id).await?;
    }
    Ok(())
}

/// Creates the income exactly once. If it cannot (e.g. the church disabled the category), the payment stays
/// "paid" with a note, and the next refresh tries again.
async fn record_income(state: &SharedState, id: Uuid) -> AppResult<()> {
    let mut tx = state.db.begin().await?;
    let Some(p) = repo::lock(&mut tx, id).await? else { return Ok(()) };
    if p.income_id.is_some() || p.status != "paid" {
        return Ok(());
    }
    let (Some(member), Some(settings)) = (p.member_id, repo::settings(&state.db, p.foc_id).await?) else {
        drop(tx);
        repo::set_error(&state.db, id, "Paid, but the income could not be recorded (payment settings missing).").await?;
        return Ok(());
    };
    let (Some(cat), Some(method)) = (settings.category_id, settings.method_id) else {
        drop(tx);
        repo::set_error(&state.db, id, "Paid, but no income category/method is set for online payments.").await?;
        return Ok(());
    };
    let note = format!("Online payment ({}){}", p.method, p.purpose.as_deref().map(|x| format!(": {x}")).unwrap_or_default());
    match finance_service::record_online_income(&mut tx, p.foc_id, member, p.currency_id, cat, method, p.amount, note).await {
        Ok(income) => {
            repo::set_income(&mut tx, id, income).await?;
            tx.commit().await?;
        }
        Err(e) => {
            drop(tx);
            tracing::error!(error = %e, payment = %id, "paid but income not recorded");
            repo::set_error(&state.db, id, &format!("Paid, but the income could not be recorded: {e}")).await?;
        }
    }
    Ok(())
}

// ── Paynow calling us (public) ──

/// The message is only a nudge: check its signature, then ask Paynow for the real status.
pub async fn webhook(state: &SharedState, id: Uuid, body: &str) -> AppResult<()> {
    let p = repo::find(&state.db, id).await?.ok_or(PaymentError::NotFound)?;
    let (_, key) = creds(state, p.foc_id).await?;
    if !paynow::verify(&paynow::parse(body), &key) {
        return Err(AppError::Validation("Bad signature.".into()));
    }
    refresh(state, &p).await
}
