use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde_json::Value;
use sqlx::{FromRow, PgConnection, PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::models::finance::{Kind, ValidChanges, ValidEntry};

// Amounts leave the database as TEXT so no digit is ever lost to floating point.

const INCOME_SELECT: &str = "SELECT to_jsonb(i) || jsonb_build_object(
    'amount', i.amount::text, 'exchange_rate', i.exchange_rate::text, 'amount_usd', i.amount_usd::text,
    'currency_name', cu.name, 'currency_symbol', cu.symbol, 'category_name', ca.name, 'method_name', me.name,
    'member_name', COALESCE(i.payer_name, NULLIF(btrim(concat_ws(' ', m.first_name, m.last_name)), '')))
  FROM incomes i
  LEFT JOIN currencies cu ON cu.id = i.currency_id
  LEFT JOIN categories ca ON ca.id = i.category_id
  LEFT JOIN methods me ON me.id = i.method_id
  LEFT JOIN member_profiles m ON m.id = i.member_id";

const EXPENSE_SELECT: &str = "SELECT to_jsonb(i) || jsonb_build_object(
    'amount', i.amount::text, 'exchange_rate', i.exchange_rate::text, 'amount_usd', i.amount_usd::text,
    'currency_name', cu.name, 'currency_symbol', cu.symbol, 'category_name', ca.name, 'method_name', me.name,
    'ministry_name', mi.name)
  FROM expenses i
  LEFT JOIN currencies cu ON cu.id = i.currency_id
  LEFT JOIN categories ca ON ca.id = i.category_id
  LEFT JOIN methods me ON me.id = i.method_id
  LEFT JOIN ministries mi ON mi.id = i.ministry_id";

fn select_sql(kind: Kind) -> &'static str {
    match kind {
        Kind::Income => INCOME_SELECT,
        Kind::Expense => EXPENSE_SELECT,
    }
}

// ── what a FOC has enabled ──

pub async fn currency_for_foc(
    conn: &mut PgConnection, foc: Uuid, currency: Uuid,
) -> Result<Option<(Option<Decimal>, String)>, sqlx::Error> {
    sqlx::query_as("SELECT fc.rate, c.name FROM foc_currencies fc JOIN currencies c ON c.id = fc.currency_id
                    WHERE fc.foc_id = $1 AND fc.currency_id = $2")
        .bind(foc).bind(currency).fetch_optional(conn).await
}

/// The category's type ("income"/"expense") if this FOC has it enabled.
pub async fn category_type_for_foc(conn: &mut PgConnection, foc: Uuid, category: Uuid) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT c.type FROM foc_categories fc JOIN categories c ON c.id = fc.category_id
                        WHERE fc.foc_id = $1 AND fc.category_id = $2")
        .bind(foc).bind(category).fetch_optional(conn).await
}

pub async fn method_enabled(conn: &mut PgConnection, foc: Uuid, method: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM foc_methods WHERE foc_id = $1 AND method_id = $2)")
        .bind(foc).bind(method).fetch_one(conn).await
}

pub async fn ministry_enabled(conn: &mut PgConnection, foc: Uuid, ministry: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM foc_ministries WHERE foc_id = $1 AND ministry_id = $2)")
        .bind(foc).bind(ministry).fetch_one(conn).await
}

pub async fn member_in_foc(conn: &mut PgConnection, member: Uuid, foc: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM member_profiles WHERE id = $1 AND foc_id = $2 AND deleted_at IS NULL)")
        .bind(member).bind(foc).fetch_one(conn).await
}

// ── ledger entries ──

pub async fn insert_entry(conn: &mut PgConnection, kind: Kind, foc: Uuid, by: Uuid, e: &ValidEntry) -> Result<Uuid, sqlx::Error> {
    match kind {
        Kind::Income => {
            sqlx::query_scalar(
                "INSERT INTO incomes (foc_id, member_id, event_id, category_id, amount, currency_id, method_id,
                                      description, date, exchange_rate, recorded_by, pledge_id, payer_name)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13) RETURNING id")
                .bind(foc).bind(e.member_id).bind(e.event_id).bind(e.category_id).bind(e.amount)
                .bind(e.currency_id).bind(e.method_id).bind(&e.description).bind(e.date).bind(e.rate).bind(by).bind(e.pledge_id).bind(&e.payer_name)
                .fetch_one(conn).await
        }
        Kind::Expense => {
            sqlx::query_scalar(
                "INSERT INTO expenses (foc_id, supplier, ministry_id, category_id, amount, currency_id, method_id,
                                       description, date, exchange_rate, recorded_by)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) RETURNING id")
                .bind(foc).bind(&e.supplier).bind(e.ministry_id).bind(e.category_id).bind(e.amount)
                .bind(e.currency_id).bind(e.method_id).bind(&e.description).bind(e.date).bind(e.rate).bind(by)
                .fetch_one(conn).await
        }
    }
}

pub async fn find_entry(conn: &mut PgConnection, kind: Kind, id: Uuid, foc: Uuid) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar(&format!("{} WHERE i.id = $1 AND i.foc_id = $2", select_sql(kind)))
        .bind(id).bind(foc).fetch_optional(conn).await
}

#[derive(Debug, FromRow)]
pub struct Current {
    pub currency_id: Uuid,
    pub voided: bool,
}

/// Locks the row for the rest of the transaction, so two edits cannot interleave.
pub async fn lock_entry(conn: &mut PgConnection, kind: Kind, id: Uuid, foc: Uuid) -> Result<Option<Current>, sqlx::Error> {
    sqlx::query_as(&format!(
        "SELECT currency_id, (voided_at IS NOT NULL) AS voided FROM {} WHERE id = $1 AND foc_id = $2 FOR UPDATE",
        kind.table()))
        .bind(id).bind(foc).fetch_optional(conn).await
}

pub struct EntryFilter {
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub category_id: Option<Uuid>,
    pub member_id: Option<Uuid>,
    pub event_id: Option<Uuid>,
    pub include_voided: bool,
    pub limit: i64,
}

pub async fn list_entries(pool: &PgPool, kind: Kind, foc: Uuid, f: &EntryFilter) -> Result<Vec<Value>, sqlx::Error> {
    let mut qb = QueryBuilder::<Postgres>::new(select_sql(kind));
    qb.push(" WHERE i.foc_id = ").push_bind(foc);
    qb.push(" AND i.date BETWEEN ").push_bind(f.start).push(" AND ").push_bind(f.end);
    if !f.include_voided {
        qb.push(" AND i.voided_at IS NULL");
    }
    if let Some(c) = f.category_id {
        qb.push(" AND i.category_id = ").push_bind(c);
    }
    if kind == Kind::Income {
        if let Some(m) = f.member_id {
            qb.push(" AND i.member_id = ").push_bind(m);
        }
        if let Some(e) = f.event_id {
            qb.push(" AND i.event_id = ").push_bind(e);
        }
    }
    qb.push(" ORDER BY i.date DESC, i.created_at DESC LIMIT ").push_bind(f.limit);
    qb.build_query_scalar::<Value>().fetch_all(pool).await
}

pub async fn update_entry(conn: &mut PgConnection, kind: Kind, id: Uuid, foc: Uuid, c: &ValidChanges) -> Result<(), sqlx::Error> {
    let mut qb = QueryBuilder::<Postgres>::new(format!("UPDATE {} SET updated_at = now()", kind.table()));
    if let Some(v) = c.amount {
        qb.push(", amount = ").push_bind(v);
    }
    if let Some((currency, rate)) = c.currency {
        qb.push(", currency_id = ").push_bind(currency).push(", exchange_rate = ").push_bind(rate);
    }
    if let Some(v) = c.category_id {
        qb.push(", category_id = ").push_bind(v);
    }
    if let Some(v) = c.method_id {
        qb.push(", method_id = ").push_bind(v);
    }
    if let Some(v) = &c.description {
        qb.push(", description = ").push_bind(v.clone());
    }
    if let Some(v) = c.date {
        qb.push(", date = ").push_bind(v);
    }
    match kind {
        Kind::Income => {
            if let Some(v) = c.member_id {
                qb.push(", member_id = ").push_bind(v);
            }
        }
        Kind::Expense => {
            if let Some(v) = &c.supplier {
                qb.push(", supplier = ").push_bind(v.clone());
            }
            if let Some(v) = c.ministry_id {
                qb.push(", ministry_id = ").push_bind(v);
            }
        }
    }
    qb.push(" WHERE id = ").push_bind(id).push(" AND foc_id = ").push_bind(foc);
    qb.build().execute(conn).await?;
    Ok(())
}

pub async fn void_entry(conn: &mut PgConnection, kind: Kind, id: Uuid, foc: Uuid, by: Uuid, reason: &str) -> Result<(), sqlx::Error> {
    sqlx::query(&format!(
        "UPDATE {} SET voided_at = now(), voided_by = $3, void_reason = $4, updated_at = now()
         WHERE id = $1 AND foc_id = $2 AND voided_at IS NULL", kind.table()))
        .bind(id).bind(foc).bind(by).bind(reason).execute(conn).await?;
    Ok(())
}

// ── audit trail ──

#[allow(clippy::too_many_arguments)]
pub async fn insert_audit(
    conn: &mut PgConnection, foc: Uuid, kind: Kind, record: Uuid, action: &str, by: Uuid,
    reason: Option<&str>, before: Option<&Value>, after: Option<&Value>,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO finance_audit (foc_id, record_type, record_id, action, changed_by, reason, before_data, after_data)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(foc).bind(kind.name()).bind(record).bind(action).bind(by).bind(reason).bind(before).bind(after)
        .execute(conn).await?;
    Ok(())
}

pub async fn history(pool: &PgPool, foc: Uuid, record: Uuid) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar("SELECT to_jsonb(a) FROM finance_audit a WHERE a.foc_id = $1 AND a.record_id = $2 ORDER BY a.created_at DESC")
        .bind(foc).bind(record).fetch_all(pool).await
}

// ── reports (voided records are excluded) ──

pub async fn summary(pool: &PgPool, kind: Kind, foc: Uuid, start: NaiveDate, end: NaiveDate) -> Result<Value, sqlx::Error> {
    let t = kind.table();
    let by = |join: &str, key: &str, label: &str| format!(
        "(SELECT COALESCE(jsonb_agg(x ORDER BY x->>'{label}'), '[]'::jsonb) FROM (
            SELECT jsonb_build_object('{label}', d.name, {key}
                   'total', SUM(e.amount)::text, 'total_usd', COALESCE(SUM(e.amount_usd), 0)::text, 'count', COUNT(*)) AS x
            FROM {t} e JOIN {join} d ON d.id = e.{col}
            WHERE e.foc_id = $1 AND e.date BETWEEN $2 AND $3 AND e.voided_at IS NULL
            GROUP BY d.name{grp}) s)",
        col = match join { "currencies" => "currency_id", "categories" => "category_id", _ => "method_id" },
        grp = if join == "currencies" { ", d.symbol" } else { "" });
    let sql = format!(
        "SELECT jsonb_build_object(
            'total_usd', COALESCE(SUM(t.amount_usd), 0)::text,
            'count', COUNT(*),
            'unconverted', COUNT(*) FILTER (WHERE t.amount_usd IS NULL),
            'by_currency', {cur}, 'by_category', {cat}, 'by_method', {met})
         FROM {t} t WHERE t.foc_id = $1 AND t.date BETWEEN $2 AND $3 AND t.voided_at IS NULL",
        cur = by("currencies", "'symbol', d.symbol,", "currency"),
        cat = by("categories", "", "category"),
        met = by("methods", "", "method"));
    sqlx::query_scalar(&sql).bind(foc).bind(start).bind(end).fetch_one(pool).await
}

// ── catalogue set-up ──

pub async fn setup(pool: &PgPool, foc: Uuid) -> Result<Value, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object(
           'base_currency', 'USD',
           'currencies', (SELECT COALESCE(jsonb_agg(jsonb_build_object('id', c.id, 'name', c.name, 'symbol', c.symbol,
                'selected', fc.currency_id IS NOT NULL, 'rate', fc.rate::text, 'rate_updated_at', fc.rate_updated_at) ORDER BY c.name), '[]'::jsonb)
              FROM currencies c LEFT JOIN foc_currencies fc ON fc.currency_id = c.id AND fc.foc_id = $1),
           'categories', (SELECT COALESCE(jsonb_agg(jsonb_build_object('id', c.id, 'name', c.name, 'type', c.type,
                'description', c.description, 'selected', fc.category_id IS NOT NULL) ORDER BY c.type, c.name), '[]'::jsonb)
              FROM categories c LEFT JOIN foc_categories fc ON fc.category_id = c.id AND fc.foc_id = $1),
           'methods', (SELECT COALESCE(jsonb_agg(jsonb_build_object('id', m.id, 'name', m.name, 'description', m.description,
                'selected', fm.method_id IS NOT NULL) ORDER BY m.name), '[]'::jsonb)
              FROM methods m LEFT JOIN foc_methods fm ON fm.method_id = m.id AND fm.foc_id = $1))")
        .bind(foc).fetch_one(pool).await
}

pub async fn currency_name(pool: &PgPool, id: Uuid) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT name FROM currencies WHERE id = $1").bind(id).fetch_optional(pool).await
}

pub async fn select_currency(pool: &PgPool, foc: Uuid, id: Uuid, rate: Option<Decimal>) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO foc_currencies (foc_id, currency_id, rate, rate_updated_at) VALUES ($1, $2, $3, CASE WHEN $3::numeric IS NOT NULL THEN now() END)
                 ON CONFLICT (foc_id, currency_id) DO UPDATE SET rate = COALESCE(EXCLUDED.rate, foc_currencies.rate),
                     rate_updated_at = CASE WHEN EXCLUDED.rate IS NOT NULL AND EXCLUDED.rate IS DISTINCT FROM foc_currencies.rate
                                            THEN now() ELSE foc_currencies.rate_updated_at END")
        .bind(foc).bind(id).bind(rate).execute(pool).await?;
    Ok(())
}

pub async fn deselect(pool: &PgPool, table: &'static str, col: &'static str, foc: Uuid, id: Uuid) -> Result<(), sqlx::Error> {
    // `table`/`col` are compile-time constants chosen by our own code, never user input
    sqlx::query(&format!("DELETE FROM {table} WHERE foc_id = $1 AND {col} = $2")).bind(foc).bind(id).execute(pool).await?;
    Ok(())
}

pub async fn create_currency(pool: &PgPool, name: &str, symbol: &str) -> Result<Uuid, sqlx::Error> {
    sqlx::query("INSERT INTO currencies (name, symbol) VALUES ($1, $2) ON CONFLICT ((upper(name))) DO NOTHING")
        .bind(name).bind(symbol).execute(pool).await?;
    sqlx::query_scalar("SELECT id FROM currencies WHERE upper(name) = upper($1)").bind(name).fetch_one(pool).await
}

/// Returns (id, type). Names are normalised by the table's own trigger.
pub async fn create_category(pool: &PgPool, name: &str, kind: &str, description: Option<&str>) -> Result<(Uuid, String), sqlx::Error> {
    sqlx::query("INSERT INTO categories (name, type, description) VALUES ($1, $2, $3) ON CONFLICT ((lower(name))) DO NOTHING")
        .bind(name).bind(kind).bind(description).execute(pool).await?;
    sqlx::query_as("SELECT id, type FROM categories WHERE lower(name) = lower($1)").bind(name).fetch_one(pool).await
}

pub async fn select_category(pool: &PgPool, foc: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM categories WHERE id = $1)").bind(id).fetch_one(pool).await?;
    if exists {
        sqlx::query("INSERT INTO foc_categories (foc_id, category_id) VALUES ($1, $2) ON CONFLICT (foc_id, category_id) DO NOTHING")
            .bind(foc).bind(id).execute(pool).await?;
    }
    Ok(exists)
}

pub async fn create_method(pool: &PgPool, name: &str, description: Option<&str>) -> Result<Uuid, sqlx::Error> {
    sqlx::query("INSERT INTO methods (name, description) VALUES ($1, $2) ON CONFLICT ((lower(name))) WHERE name IS NOT NULL DO NOTHING")
        .bind(name).bind(description).execute(pool).await?;
    sqlx::query_scalar("SELECT id FROM methods WHERE lower(name) = lower($1)").bind(name).fetch_one(pool).await
}

pub async fn select_method(pool: &PgPool, foc: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM methods WHERE id = $1)").bind(id).fetch_one(pool).await?;
    if exists {
        sqlx::query("INSERT INTO foc_methods (foc_id, method_id) VALUES ($1, $2) ON CONFLICT (foc_id, method_id) DO NOTHING")
            .bind(foc).bind(id).execute(pool).await?;
    }
    Ok(exists)
}

/// How many incomes + expenses of this FOC use a currency / category / method.
/// `column` is one of our own literals, never user input.
pub async fn usage_count(pool: &PgPool, foc_id: Uuid, column: &str, id: Uuid) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(&format!(
        "SELECT (SELECT count(*) FROM incomes WHERE foc_id = $1 AND {column} = $2)
              + (SELECT count(*) FROM expenses WHERE foc_id = $1 AND {column} = $2)"
    ))
    .bind(foc_id)
    .bind(id)
    .fetch_one(pool)
    .await
}
