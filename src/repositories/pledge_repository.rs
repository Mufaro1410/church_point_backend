use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde_json::Value;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

type R<T> = Result<T, sqlx::Error>;

/// A pledge with what has been given towards it so far.
/// Giving counts when it is attached to the pledge, or (for a member's pledge) when it is that member's giving
/// in the same fund and currency inside the pledge dates.
pub(crate) const VIEW: &str = "SELECT to_jsonb(p) || jsonb_build_object(
        'pledger_type', CASE WHEN p.member_id IS NOT NULL THEN 'member' WHEN p.ministry_id IS NOT NULL THEN 'ministry' ELSE 'other' END,
        'pledger_name', coalesce(nullif(btrim(coalesce(m.first_name,'') || ' ' || coalesce(m.last_name,'')), ''), mi.name, p.pledger_name),
        'category_name', c.name, 'currency', cu.name,
        'amount', p.amount::text, 'paid', g.paid::text,
        'remaining', greatest(p.amount - g.paid, 0)::numeric(14,2)::text,
        'fulfilled', g.paid >= p.amount,
        'overdue', p.status = 'active' AND g.paid < p.amount AND p.end_date IS NOT NULL AND p.end_date < current_date)
    FROM pledges p
    LEFT JOIN member_profiles m ON m.id = p.member_id
    LEFT JOIN ministries mi ON mi.id = p.ministry_id
    JOIN categories c ON c.id = p.category_id
    JOIN currencies cu ON cu.id = p.currency_id
    CROSS JOIN LATERAL (
        SELECT coalesce(sum(i.amount), 0)::numeric(14,2) AS paid FROM incomes i
        WHERE i.foc_id = p.foc_id AND i.voided_at IS NULL AND (
            i.pledge_id = p.id
            OR (p.member_id IS NOT NULL AND i.pledge_id IS NULL AND i.member_id = p.member_id AND i.category_id = p.category_id
                AND i.currency_id = p.currency_id AND i.date >= p.start_date AND (p.end_date IS NULL OR i.date <= p.end_date)))) g";

pub async fn list(pool: &PgPool, foc: Uuid, member: Option<Uuid>) -> R<Vec<Value>> {
    sqlx::query_scalar(&format!("{VIEW} WHERE p.foc_id = $1 AND ($2::uuid IS NULL OR p.member_id = $2) ORDER BY p.created_at DESC"))
        .bind(foc).bind(member).fetch_all(pool).await
}

pub async fn get(pool: &PgPool, foc: Uuid, id: Uuid) -> R<Option<Value>> {
    sqlx::query_scalar(&format!("{VIEW} WHERE p.foc_id = $1 AND p.id = $2")).bind(foc).bind(id).fetch_optional(pool).await
}

#[allow(clippy::too_many_arguments)]
pub async fn insert(conn: &mut PgConnection, foc: Uuid, by: Uuid, member: Option<Uuid>, ministry: Option<Uuid>, other: Option<&str>, category: Uuid, currency: Uuid,
                    amount: Decimal, start: Option<NaiveDate>, end: Option<NaiveDate>, note: Option<&str>) -> R<Uuid> {
    sqlx::query_scalar(
        "INSERT INTO pledges (foc_id, member_id, ministry_id, pledger_name, category_id, currency_id, amount, start_date, end_date, note, created_by)
         VALUES ($1,$2,$3,$4,$5,$6,$7, COALESCE($8, current_date), $9, $10, $11) RETURNING id")
        .bind(foc).bind(member).bind(ministry).bind(other).bind(category).bind(currency).bind(amount).bind(start).bind(end).bind(note).bind(by)
        .fetch_one(conn).await
}

pub async fn update(pool: &PgPool, foc: Uuid, id: Uuid, amount: Option<Decimal>, end: Option<NaiveDate>, status: Option<&str>, note: Option<&str>) -> R<bool> {
    Ok(sqlx::query(
        "UPDATE pledges SET amount = COALESCE($3, amount), end_date = COALESCE($4, end_date), status = COALESCE($5, status),
                note = COALESCE($6, note), updated_at = now() WHERE id = $2 AND foc_id = $1")
        .bind(foc).bind(id).bind(amount).bind(end).bind(status).bind(note).execute(pool).await?.rows_affected() > 0)
}

/// Can giving in this fund and currency be attached to this pledge?
pub async fn accepts(conn: &mut PgConnection, foc: Uuid, pledge: Uuid, currency: Uuid, category: Uuid) -> R<bool> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pledges WHERE id = $2 AND foc_id = $1 AND status = 'active' AND currency_id = $3 AND category_id = $4)")
        .bind(foc).bind(pledge).bind(currency).bind(category).fetch_one(conn).await
}
