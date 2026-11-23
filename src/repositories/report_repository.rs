//! Read-only report queries. Each returns JSON ready for the web pages; money stays text so nothing is rounded twice.

use chrono::NaiveDate;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

type R<T> = Result<T, sqlx::Error>;

/// A number from an event's `manual_stats`, 0 when missing or not a number.
fn stat(key: &str) -> String {
    format!("(CASE WHEN (e.manual_stats ->> '{key}') ~ '^[0-9]+$' THEN (e.manual_stats ->> '{key}')::int ELSE 0 END)")
}
/// How many people were counted in the room (the same sum the event page shows).
fn headcount() -> String {
    format!("({} + {} + {} + {})", stat("males"), stat("females"), stat("arena"), stat("children"))
}
const CHECKINS: &str = "(SELECT count(*) FROM jsonb_object_keys(coalesce(e.attendance, '{}'::jsonb)))";

// ───────────── overview ─────────────

pub async fn members(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate, pfrom: NaiveDate, pto: NaiveDate) -> R<Value> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object(
            'total', count(*) FILTER (WHERE status = 'approved' AND deleted_at IS NULL),
            'active', count(*) FILTER (WHERE status = 'approved' AND deleted_at IS NULL AND membership_status = 'active'),
            'pending', count(*) FILTER (WHERE status = 'pending' AND deleted_at IS NULL),
            'not_in_cell', count(*) FILTER (WHERE status = 'approved' AND deleted_at IS NULL AND cell_group_id IS NULL),
            'joined', count(*) FILTER (WHERE status IN ('approved','transfer_requested') AND coalesce(date_joined, created_at::date) BETWEEN $2 AND $3),
            'joined_prev', count(*) FILTER (WHERE status IN ('approved','transfer_requested') AND coalesce(date_joined, created_at::date) BETWEEN $4 AND $5),
            'left', count(*) FILTER (WHERE deleted_at::date BETWEEN $2 AND $3),
            'left_prev', count(*) FILTER (WHERE deleted_at::date BETWEEN $4 AND $5))
         FROM member_profiles WHERE foc_id = $1")
        .bind(foc).bind(from).bind(to).bind(pfrom).bind(pto).fetch_one(pool).await
}

/// Services in a period: how many, and the average people per service by check-in and by headcount.
pub async fn attendance_summary(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate) -> R<Value> {
    sqlx::query_scalar(&format!(
        "SELECT jsonb_build_object('services', count(*), 'checkins', coalesce(sum(c), 0),
                'avg_checkins', coalesce(round(avg(c) FILTER (WHERE c > 0), 1), 0),
                'avg_headcount', coalesce(round(avg(h) FILTER (WHERE h > 0), 1), 0))
         FROM (SELECT {CHECKINS} AS c, {} AS h FROM get_visible_events($1, $2, $3) e WHERE e.date <= current_date) t",
        headcount()))
        .bind(foc).bind(from).bind(to).fetch_one(pool).await
}

/// Totals per currency for a period and the one before it. `table` is "incomes" or "expenses".
pub async fn money(pool: &PgPool, table: &str, foc: Uuid, from: NaiveDate, to: NaiveDate, pfrom: NaiveDate) -> R<Vec<Value>> {
    let givers = if table == "incomes" { "count(DISTINCT t.member_id) FILTER (WHERE t.date BETWEEN $2 AND $3)" } else { "0" };
    sqlx::query_scalar(&format!(
        "SELECT jsonb_build_object('currency', cu.name,
                'current', coalesce(sum(t.amount) FILTER (WHERE t.date BETWEEN $2 AND $3), 0)::numeric(14,2)::text,
                'previous', coalesce(sum(t.amount) FILTER (WHERE t.date BETWEEN $4 AND $2 - 1), 0)::numeric(14,2)::text,
                'count', count(*) FILTER (WHERE t.date BETWEEN $2 AND $3),
                'givers', {givers})
         FROM {table} t JOIN currencies cu ON cu.id = t.currency_id
         WHERE t.foc_id = $1 AND t.voided_at IS NULL AND t.date BETWEEN $4 AND $3
         GROUP BY cu.name ORDER BY cu.name"))
        .bind(foc).bind(from).bind(to).bind(pfrom).fetch_all(pool).await
}

pub async fn visitors(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate, pfrom: NaiveDate, pto: NaiveDate) -> R<Value> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object(
            'count', count(*) FILTER (WHERE visit_date BETWEEN $2 AND $3),
            'count_prev', count(*) FILTER (WHERE visit_date BETWEEN $4 AND $5),
            'joined', count(*) FILTER (WHERE visit_date BETWEEN $2 AND $3 AND status = 'joined'),
            'self_signed', count(*) FILTER (WHERE visit_date BETWEEN $2 AND $3 AND source = 'form'),
            'waiting', count(*) FILTER (WHERE status = 'new'))
         FROM visitors WHERE foc_id = $1")
        .bind(foc).bind(from).bind(to).bind(pfrom).bind(pto).fetch_one(pool).await
}

// ───────────── attendance ─────────────

pub async fn services(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate) -> R<Vec<Value>> {
    sqlx::query_scalar(&format!(
        "SELECT jsonb_build_object('id', e.id, 'title', e.title, 'date', e.date, 'checkins', {CHECKINS}, 'headcount', {},
                'visitors', {}, 'salvations', {})
         FROM get_visible_events($1, $2, $3) e WHERE e.date <= current_date ORDER BY e.date, e.start_time",
        headcount(), stat("visitors"), stat("salvation")))
        .bind(foc).bind(from).bind(to).fetch_all(pool).await
}

/// How often members were checked in, over services that had any check-ins at all.
pub async fn regularity(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate) -> R<Value> {
    sqlx::query_scalar(
        "WITH ev AS (SELECT e.attendance FROM get_visible_events($1, $2, $3) e
                     WHERE e.date <= current_date AND e.attendance IS NOT NULL AND e.attendance <> '{}'::jsonb),
              n AS (SELECT count(*)::numeric AS c FROM ev),
              k AS (SELECT m.id, count(*) FILTER (WHERE ev.attendance ? m.id::text)::numeric AS k
                    FROM member_profiles m CROSS JOIN ev
                    WHERE m.foc_id = $1 AND m.status = 'approved' AND m.deleted_at IS NULL GROUP BY m.id)
         SELECT jsonb_build_object('services_with_checkins', (SELECT c FROM n)::int,
                'regular', count(*) FILTER (WHERE k >= 0.75 * (SELECT c FROM n)),
                'occasional', count(*) FILTER (WHERE k >= 0.25 * (SELECT c FROM n) AND k < 0.75 * (SELECT c FROM n)),
                'rare', count(*) FILTER (WHERE k > 0 AND k < 0.25 * (SELECT c FROM n)),
                'none', count(*) FILTER (WHERE k = 0))
         FROM k")
        .bind(foc).bind(from).bind(to).fetch_one(pool).await
}

/// Members who were checked in during the previous period but not at all in this one.
pub async fn stopped(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate, pfrom: NaiveDate, pto: NaiveDate) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "WITH cur AS (SELECT e.attendance FROM get_visible_events($1, $2, $3) e WHERE e.date <= current_date AND e.attendance IS NOT NULL),
              prev AS (SELECT e.attendance FROM get_visible_events($1, $4, $5) e WHERE e.attendance IS NOT NULL)
         SELECT jsonb_build_object('id', m.id, 'name', btrim(coalesce(m.first_name,'') || ' ' || coalesce(m.last_name,'')), 'phone', m.phone,
                'attended_before', (SELECT count(*) FROM prev WHERE prev.attendance ? m.id::text))
         FROM member_profiles m
         WHERE m.foc_id = $1 AND m.status = 'approved' AND m.deleted_at IS NULL
           AND EXISTS (SELECT 1 FROM prev WHERE prev.attendance ? m.id::text)
           AND NOT EXISTS (SELECT 1 FROM cur WHERE cur.attendance ? m.id::text)
         ORDER BY (SELECT count(*) FROM prev WHERE prev.attendance ? m.id::text) DESC, m.first_name LIMIT 100")
        .bind(foc).bind(from).bind(to).bind(pfrom).bind(pto).fetch_all(pool).await
}

// ───────────── giving ─────────────

/// Totals per currency (and one USD total from each record's saved rate), for this period and the one before.
pub async fn giving_totals(pool: &PgPool, table: &str, foc: Uuid, from: NaiveDate, to: NaiveDate, pfrom: NaiveDate) -> R<Vec<Value>> {
    let extra = if table == "incomes" {
        "'givers', count(DISTINCT t.member_id) FILTER (WHERE t.date BETWEEN $2 AND $3),
         'givers_prev', count(DISTINCT t.member_id) FILTER (WHERE t.date BETWEEN $4 AND $2 - 1),
         'largest', coalesce(max(t.amount) FILTER (WHERE t.date BETWEEN $2 AND $3), 0)::numeric(14,2)::text,"
    } else { "" };
    sqlx::query_scalar(&format!(
        "SELECT jsonb_build_object('currency', cu.name, {extra}
                'current', coalesce(sum(t.amount) FILTER (WHERE t.date BETWEEN $2 AND $3), 0)::numeric(14,2)::text,
                'previous', coalesce(sum(t.amount) FILTER (WHERE t.date BETWEEN $4 AND $2 - 1), 0)::numeric(14,2)::text,
                'count', count(*) FILTER (WHERE t.date BETWEEN $2 AND $3),
                'count_prev', count(*) FILTER (WHERE t.date BETWEEN $4 AND $2 - 1),
                'usd', coalesce(sum(t.amount_usd) FILTER (WHERE t.date BETWEEN $2 AND $3), 0)::numeric(14,2)::text,
                'usd_prev', coalesce(sum(t.amount_usd) FILTER (WHERE t.date BETWEEN $4 AND $2 - 1), 0)::numeric(14,2)::text)
         FROM {table} t JOIN currencies cu ON cu.id = t.currency_id
         WHERE t.foc_id = $1 AND t.voided_at IS NULL AND t.date BETWEEN $4 AND $3
         GROUP BY cu.name ORDER BY cu.name"))
        .bind(foc).bind(from).bind(to).bind(pfrom).fetch_all(pool).await
}

/// Split by category or by payment method: per currency, this period against the one before.
pub async fn split(pool: &PgPool, table: &str, by: &str, foc: Uuid, from: NaiveDate, to: NaiveDate, pfrom: NaiveDate) -> R<Vec<Value>> {
    let (join, name) = match by { "method" => ("JOIN methods x ON x.id = t.method_id", "x.name"), _ => ("JOIN categories x ON x.id = t.category_id", "x.name") };
    sqlx::query_scalar(&format!(
        "SELECT jsonb_build_object('name', {name}, 'currency', cu.name,
                'current', coalesce(sum(t.amount) FILTER (WHERE t.date BETWEEN $2 AND $3), 0)::numeric(14,2)::text,
                'previous', coalesce(sum(t.amount) FILTER (WHERE t.date BETWEEN $4 AND $2 - 1), 0)::numeric(14,2)::text,
                'count', count(*) FILTER (WHERE t.date BETWEEN $2 AND $3))
         FROM {table} t {join} JOIN currencies cu ON cu.id = t.currency_id
         WHERE t.foc_id = $1 AND t.voided_at IS NULL AND t.date BETWEEN $4 AND $3
         GROUP BY {name}, cu.name HAVING count(*) FILTER (WHERE t.date BETWEEN $2 AND $3) > 0
         ORDER BY cu.name, sum(t.amount) FILTER (WHERE t.date BETWEEN $2 AND $3) DESC"))
        .bind(foc).bind(from).bind(to).bind(pfrom).fetch_all(pool).await
}

/// Month by month, per currency. Months with nothing are simply absent.
pub async fn monthly(pool: &PgPool, table: &str, foc: Uuid, from: NaiveDate, to: NaiveDate) -> R<Vec<Value>> {
    sqlx::query_scalar(&format!(
        "SELECT jsonb_build_object('month', to_char(date_trunc('month', t.date), 'YYYY-MM'), 'currency', cu.name, 'total', sum(t.amount)::numeric(14,2)::text)
         FROM {table} t JOIN currencies cu ON cu.id = t.currency_id
         WHERE t.foc_id = $1 AND t.voided_at IS NULL AND t.date BETWEEN $2 AND $3
         GROUP BY date_trunc('month', t.date), cu.name ORDER BY date_trunc('month', t.date)"))
        .bind(foc).bind(from).bind(to).fetch_all(pool).await
}

/// Who gave for the first time ever in this period, and who gave in the period before but not in this one.
pub async fn giver_movement(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate, pfrom: NaiveDate) -> R<Value> {
    sqlx::query_scalar(
        "WITH g AS (SELECT member_id, min(date) AS first, bool_or(date BETWEEN $2 AND $3) AS now, bool_or(date BETWEEN $4 AND $2 - 1) AS before
                    FROM incomes WHERE foc_id = $1 AND voided_at IS NULL AND member_id IS NOT NULL GROUP BY member_id)
         SELECT jsonb_build_object('new_givers', count(*) FILTER (WHERE first BETWEEN $2 AND $3),
                'lapsed', count(*) FILTER (WHERE before AND NOT now),
                'kept', count(*) FILTER (WHERE before AND now),
                'members_giving', count(*) FILTER (WHERE now))
         FROM g")
        .bind(foc).bind(from).bind(to).bind(pfrom).fetch_one(pool).await
}

/// Active members who have given in this period, as a share of all active members.
pub async fn giving_share(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate) -> R<Value> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object('members', count(*),
                'giving', count(*) FILTER (WHERE EXISTS (SELECT 1 FROM incomes i WHERE i.member_id = m.id AND i.voided_at IS NULL AND i.date BETWEEN $2 AND $3)))
         FROM member_profiles m WHERE m.foc_id = $1 AND m.status = 'approved' AND m.deleted_at IS NULL")
        .bind(foc).bind(from).bind(to).fetch_one(pool).await
}

pub async fn top_givers(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object('id', t.member_id, 'name', t.name, 'currency', t.cur, 'total', t.total::text, 'gifts', t.gifts)
         FROM (SELECT i.member_id, btrim(coalesce(m.first_name,'') || ' ' || coalesce(m.last_name,'')) AS name, cu.name AS cur,
                      sum(i.amount)::numeric(14,2) AS total, count(*) AS gifts,
                      row_number() OVER (PARTITION BY cu.name ORDER BY sum(i.amount) DESC) AS rn
               FROM incomes i JOIN member_profiles m ON m.id = i.member_id JOIN currencies cu ON cu.id = i.currency_id
               WHERE i.foc_id = $1 AND i.voided_at IS NULL AND i.date BETWEEN $2 AND $3
               GROUP BY i.member_id, m.first_name, m.last_name, cu.name) t
         WHERE t.rn <= 10 ORDER BY t.cur, t.total DESC")
        .bind(foc).bind(from).bind(to).fetch_all(pool).await
}

/// Active pledges: how much was promised, how much has come in, and how many are late.
pub async fn pledges(pool: &PgPool, foc: Uuid) -> R<Vec<Value>> {
    sqlx::query_scalar(&format!(
        "SELECT jsonb_build_object('currency', j->>'currency', 'count', count(*),
                'pledged', sum((j->>'amount')::numeric)::numeric(14,2)::text, 'paid', sum((j->>'paid')::numeric)::numeric(14,2)::text,
                'fulfilled', count(*) FILTER (WHERE (j->>'fulfilled')::bool), 'overdue', count(*) FILTER (WHERE (j->>'overdue')::bool))
         FROM ({} WHERE p.foc_id = $1 AND p.status = 'active') t(j) GROUP BY j->>'currency' ORDER BY 1",
        super::pledge_repository::VIEW))
        .bind(foc).fetch_all(pool).await
}
