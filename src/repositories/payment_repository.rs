use rust_decimal::Decimal;
use serde_json::Value;
use sqlx::{FromRow, PgConnection, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct SettingsRow {
    pub integration_id: String,
    pub integration_key_enc: String,
    pub merchant_email: String,
    pub test_mode: bool,
    pub category_id: Option<Uuid>,
    pub method_id: Option<Uuid>,
    pub enabled: bool,
}

pub async fn settings(pool: &PgPool, foc: Uuid) -> Result<Option<SettingsRow>, sqlx::Error> {
    sqlx::query_as("SELECT integration_id, integration_key_enc, merchant_email, test_mode, category_id, method_id, enabled
                    FROM foc_payment_settings WHERE foc_id = $1")
        .bind(foc).fetch_optional(pool).await
}

pub async fn settings_json(pool: &PgPool, foc: Uuid) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar("SELECT jsonb_build_object('provider', provider, 'integration_id', integration_id, 'key_saved', true,
                               'merchant_email', merchant_email, 'test_mode', test_mode, 'category_id', category_id,
                               'method_id', method_id, 'enabled', enabled, 'updated_at', updated_at)
                        FROM foc_payment_settings WHERE foc_id = $1")
        .bind(foc).fetch_optional(pool).await
}

#[allow(clippy::too_many_arguments)]
pub async fn upsert_settings(
    pool: &PgPool, foc: Uuid, integration_id: &str, key_enc: Option<&str>, email: &str, test_mode: bool,
    category: Uuid, method: Uuid, enabled: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO foc_payment_settings (foc_id, integration_id, integration_key_enc, merchant_email, test_mode, category_id, method_id, enabled)
         VALUES ($1,$2,COALESCE($3,''),$4,$5,$6,$7,$8)
         ON CONFLICT (foc_id) DO UPDATE SET integration_id = $2,
             integration_key_enc = COALESCE($3, foc_payment_settings.integration_key_enc),
             merchant_email = $4, test_mode = $5, category_id = $6, method_id = $7, enabled = $8, updated_at = now()")
        .bind(foc).bind(integration_id).bind(key_enc).bind(email).bind(test_mode).bind(category).bind(method).bind(enabled)
        .execute(pool).await?;
    Ok(())
}

pub async fn member_email(pool: &PgPool, member: Uuid) -> Result<Option<String>, sqlx::Error> {
    Ok(sqlx::query_scalar::<_, Option<String>>("SELECT email FROM member_profiles WHERE id = $1")
        .bind(member).fetch_optional(pool).await?.flatten())
}

#[allow(clippy::too_many_arguments)]
pub async fn insert(
    pool: &PgPool, foc: Uuid, member: Uuid, amount: Decimal, currency: Uuid, purpose: Option<&str>, method: &str, phone: &str,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar("INSERT INTO payments (foc_id, member_id, amount, currency_id, purpose, method, phone)
                        VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING id")
        .bind(foc).bind(member).bind(amount).bind(currency).bind(purpose).bind(method).bind(phone)
        .fetch_one(pool).await
}

pub async fn mark_sent(pool: &PgPool, id: Uuid, reference: Option<&str>, poll_url: &str, instructions: Option<&str>) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE payments SET status = 'pending', paynow_reference = $2, poll_url = $3, instructions = $4, updated_at = now() WHERE id = $1")
        .bind(id).bind(reference).bind(poll_url).bind(instructions).execute(pool).await?;
    Ok(())
}

pub async fn mark_failed(pool: &PgPool, id: Uuid, error: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE payments SET status = 'failed', error = $2, updated_at = now() WHERE id = $1")
        .bind(id).bind(error).execute(pool).await?;
    Ok(())
}

#[derive(Debug, FromRow)]
pub struct PaymentRow {
    pub id: Uuid,
    pub foc_id: Uuid,
    pub member_id: Option<Uuid>,
    pub amount: Decimal,
    pub currency_id: Uuid,
    pub purpose: Option<String>,
    pub method: String,
    pub status: String,
    pub poll_url: Option<String>,
    pub income_id: Option<Uuid>,
}

const ROW: &str = "SELECT id, foc_id, member_id, amount, currency_id, purpose, method, status, poll_url, income_id FROM payments";

pub async fn find(pool: &PgPool, id: Uuid) -> Result<Option<PaymentRow>, sqlx::Error> {
    sqlx::query_as(&format!("{ROW} WHERE id = $1")).bind(id).fetch_optional(pool).await
}

pub async fn lock(conn: &mut PgConnection, id: Uuid) -> Result<Option<PaymentRow>, sqlx::Error> {
    sqlx::query_as(&format!("{ROW} WHERE id = $1 FOR UPDATE")).bind(id).fetch_optional(conn).await
}

pub async fn set_status(conn: &mut PgConnection, id: Uuid, status: &str, raw: &str, reference: Option<&str>, error: Option<&str>) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE payments SET status = $2, paynow_status = $3, paynow_reference = COALESCE($4, paynow_reference),
                        error = $5, updated_at = now() WHERE id = $1")
        .bind(id).bind(status).bind(raw).bind(reference).bind(error).execute(conn).await?;
    Ok(())
}

pub async fn set_income(conn: &mut PgConnection, id: Uuid, income: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE payments SET income_id = $2, error = NULL, updated_at = now() WHERE id = $1")
        .bind(id).bind(income).execute(conn).await?;
    Ok(())
}

pub async fn set_error(pool: &PgPool, id: Uuid, error: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE payments SET error = $2, updated_at = now() WHERE id = $1").bind(id).bind(error).execute(pool).await?;
    Ok(())
}

const VIEW: &str = "SELECT jsonb_build_object('id', p.id, 'amount', p.amount::text, 'currency_id', p.currency_id,
        'currency', c.name, 'symbol', c.symbol, 'purpose', p.purpose, 'method', p.method, 'phone', p.phone,
        'status', p.status, 'instructions', p.instructions, 'error', p.error, 'income_id', p.income_id,
        'member_id', p.member_id, 'member_name', trim(coalesce(m.first_name,'') || ' ' || coalesce(m.last_name,'')),
        'created_at', p.created_at, 'updated_at', p.updated_at)
    FROM payments p JOIN currencies c ON c.id = p.currency_id LEFT JOIN member_profiles m ON m.id = p.member_id";

pub async fn view(pool: &PgPool, id: Uuid) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar(&format!("{VIEW} WHERE p.id = $1")).bind(id).fetch_optional(pool).await
}

pub async fn list(pool: &PgPool, foc: Uuid, status: Option<&str>, limit: i64) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar(&format!("{VIEW} WHERE p.foc_id = $1 AND ($2::text IS NULL OR p.status = $2) ORDER BY p.created_at DESC LIMIT $3"))
        .bind(foc).bind(status).bind(limit).fetch_all(pool).await
}
