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
/// How many people were counted in the room (males, females, children and youth).
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

/// Hand counts for the period and the one before it, one entry per count.
pub async fn counter_totals(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate, pfrom: NaiveDate) -> R<Value> {
    const COUNTS: [(&str, &str); 8] = [
        ("males", "Males"), ("females", "Females"), ("children", "Children"), ("arena", "Youth (Arena)"),
        ("visitors", "Visitors"), ("salvation", "Salvation"), ("alterCall", "Altar call"), ("reassurance", "Reassurance"),
    ];
    let items = COUNTS.iter().map(|(k, label)| format!(
        "jsonb_build_object('key', '{k}', 'label', '{label}',
            'total', coalesce(sum({s}) FILTER (WHERE e.date BETWEEN $2 AND $3), 0)::int,
            'previous', coalesce(sum({s}) FILTER (WHERE e.date < $2), 0)::int)", s = stat(k))).collect::<Vec<_>>().join(", ");
    sqlx::query_scalar(&format!(
        "SELECT jsonb_build_array({items}) FROM get_visible_events($1, $4, $3) e WHERE e.date <= current_date"))
        .bind(foc).bind(from).bind(to).bind(pfrom).fetch_one(pool).await
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

// ───────────── members & growth ─────────────

/// Month by month inside the period: who joined, who left, and how many members there were at the month's end.
pub async fn growth_series(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "WITH mo AS (SELECT d::date AS start, least((d + interval '1 month - 1 day')::date, $3) AS stop
                     FROM generate_series(date_trunc('month', $2::date), date_trunc('month', $3::date), interval '1 month') d),
              m AS (SELECT coalesce(date_joined, created_at::date) AS joined, deleted_at::date AS gone FROM member_profiles
                    WHERE foc_id = $1 AND status IN ('approved','transfer_requested'))
         SELECT jsonb_build_object('month', to_char(mo.start, 'YYYY-MM'),
                'joined', (SELECT count(*) FROM m WHERE m.joined BETWEEN greatest(mo.start, $2) AND mo.stop),
                'left', (SELECT count(*) FROM m WHERE m.gone BETWEEN greatest(mo.start, $2) AND mo.stop),
                'members', (SELECT count(*) FROM m WHERE m.joined <= mo.stop AND (m.gone IS NULL OR m.gone > mo.stop)))
         FROM mo ORDER BY mo.start")
        .bind(foc).bind(from).bind(to).fetch_all(pool).await
}

/// Who the congregation is today: by gender, status and the church's own age groups.
pub async fn composition(pool: &PgPool, foc: Uuid) -> R<Value> {
    sqlx::query_scalar(
        "WITH p AS (SELECT * FROM member_profiles WHERE foc_id = $1 AND status = 'approved' AND deleted_at IS NULL),
              ag AS (SELECT g.id, g.name, g.min_age, g.max_age, g.sort_order FROM age_groups g WHERE g.foc_id = $1)
         SELECT jsonb_build_object(
            'total', (SELECT count(*) FROM p),
            'gender', coalesce((SELECT jsonb_agg(jsonb_build_object('name', coalesce(nullif(initcap(gender), ''), 'Not recorded'), 'count', c))
                       FROM (SELECT gender, count(*) c FROM p GROUP BY gender ORDER BY c DESC) t), '[]'::jsonb),
            'status', coalesce((SELECT jsonb_agg(jsonb_build_object('name', coalesce(nullif(initcap(membership_status), ''), 'Not recorded'), 'count', c))
                       FROM (SELECT membership_status, count(*) c FROM p GROUP BY membership_status ORDER BY c DESC) t), '[]'::jsonb),
            'age_groups', coalesce((SELECT jsonb_agg(jsonb_build_object('name', t.name, 'male', t.male, 'female', t.female, 'total', t.total) ORDER BY t.min_age, t.sort_order)
                       FROM (SELECT ag.name, ag.min_age, ag.sort_order,
                                    count(p.id) FILTER (WHERE lower(p.gender) = 'male') AS male, count(p.id) FILTER (WHERE lower(p.gender) = 'female') AS female, count(p.id) AS total
                             FROM ag LEFT JOIN p ON p.dob IS NOT NULL AND date_part('year', age(p.dob)) BETWEEN ag.min_age AND coalesce(ag.max_age, 200)
                             GROUP BY ag.id, ag.name, ag.min_age, ag.sort_order) t), '[]'::jsonb),
            'no_birthday', (SELECT count(*) FROM p WHERE dob IS NULL),
            'in_cell', (SELECT count(*) FROM p WHERE cell_group_id IS NOT NULL),
            'not_in_cell', (SELECT count(*) FROM p WHERE cell_group_id IS NULL),
            'in_ministry', (SELECT count(*) FROM p WHERE coalesce(array_length(ministry_ids, 1), 0) > 0))")
        .bind(foc).fetch_one(pool).await
}

/// People who joined in the period, with whether they have been seen at any service since joining (within the period or later).
pub async fn newcomers(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "WITH ev AS (SELECT e.attendance FROM get_visible_events($1, $2, current_date) e WHERE e.date <= current_date AND e.attendance IS NOT NULL)
         SELECT jsonb_build_object('id', m.id, 'name', btrim(coalesce(m.first_name,'') || ' ' || coalesce(m.last_name,'')), 'phone', m.phone,
                'joined', coalesce(m.date_joined, m.created_at::date), 'in_cell', m.cell_group_id IS NOT NULL,
                'seen', EXISTS (SELECT 1 FROM ev WHERE ev.attendance ? m.id::text))
         FROM member_profiles m
         WHERE m.foc_id = $1 AND m.status = 'approved' AND m.deleted_at IS NULL AND coalesce(m.date_joined, m.created_at::date) BETWEEN $2 AND $3
         ORDER BY coalesce(m.date_joined, m.created_at::date) DESC LIMIT 200")
        .bind(foc).bind(from).bind(to).fetch_all(pool).await
}

/// Birthdays in the next 30 days.
pub async fn birthdays(pool: &PgPool, foc: Uuid) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object('id', id, 'name', name, 'date', next, 'turning', date_part('year', next) - date_part('year', dob))
         FROM (SELECT m.id, btrim(coalesce(m.first_name,'') || ' ' || coalesce(m.last_name,'')) AS name, m.dob,
                      CASE WHEN make_date(date_part('year', current_date)::int, date_part('month', m.dob)::int, least(date_part('day', m.dob)::int, 28)) >= current_date
                           THEN make_date(date_part('year', current_date)::int, date_part('month', m.dob)::int, least(date_part('day', m.dob)::int, 28))
                           ELSE make_date(date_part('year', current_date)::int + 1, date_part('month', m.dob)::int, least(date_part('day', m.dob)::int, 28)) END AS next
               FROM member_profiles m WHERE m.foc_id = $1 AND m.status = 'approved' AND m.deleted_at IS NULL AND m.dob IS NOT NULL) t
         WHERE next <= current_date + 30 ORDER BY next, name")
        .bind(foc).fetch_all(pool).await
}

// ───────────── cells ─────────────

/// One row per cell: members, meetings, average people present, and when it last met.
/// Attendance entries may be plain ids or objects, and "present" defaults to true when not stated.
const PRESENT: &str = "(SELECT count(*) FROM jsonb_array_elements(coalesce(m.attendance, '[]'::jsonb)) x
                       WHERE coalesce((CASE WHEN jsonb_typeof(x) = 'object' THEN x ->> 'present' END)::bool, true))";

pub async fn cells(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate, pfrom: NaiveDate, pto: NaiveDate) -> R<Vec<Value>> {
    sqlx::query_scalar(&format!(
        "SELECT jsonb_build_object('id', g.id, 'name', g.name,
                'members', (SELECT count(*) FROM member_profiles p WHERE p.cell_group_id = g.id AND p.status = 'approved' AND p.deleted_at IS NULL),
                'meetings', count(t.id) FILTER (WHERE t.date BETWEEN $2 AND $3),
                'meetings_prev', count(t.id) FILTER (WHERE t.date BETWEEN $4 AND $5),
                'avg_present', coalesce(round(avg(t.present) FILTER (WHERE t.date BETWEEN $2 AND $3), 1), 0),
                'avg_present_prev', coalesce(round(avg(t.present) FILTER (WHERE t.date BETWEEN $4 AND $5), 1), 0),
                'last_meeting', (SELECT max(date) FROM cell_meetings WHERE cell_group_id = g.id))
         FROM cell_groups g
         LEFT JOIN (SELECT m.id, m.cell_group_id, m.date, {PRESENT} AS present FROM cell_meetings m WHERE m.date BETWEEN $4 AND $3) t ON t.cell_group_id = g.id
         WHERE g.foc_id = $1 GROUP BY g.id, g.name ORDER BY g.name"))
        .bind(foc).bind(from).bind(to).bind(pfrom).bind(pto).fetch_all(pool).await
}

/// Offerings taken at cell meetings in the period, per currency.
pub async fn cell_offerings(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object('currency', cu.name, 'total', sum((o ->> 'amount')::numeric)::numeric(14,2)::text)
         FROM cell_meetings m JOIN cell_groups g ON g.id = m.cell_group_id
         CROSS JOIN LATERAL jsonb_array_elements(coalesce(m.offering, '[]'::jsonb)) o
         JOIN currencies cu ON cu.id::text = o ->> 'currency_id'
         WHERE g.foc_id = $1 AND m.date BETWEEN $2 AND $3 AND (o ->> 'amount') ~ '^[0-9.]+$'
         GROUP BY cu.name ORDER BY cu.name")
        .bind(foc).bind(from).bind(to).fetch_all(pool).await
}

/// Members who belong to a cell but were not marked present at any of its meetings in the period.
pub async fn cell_absentees(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object('id', p.id, 'name', btrim(coalesce(p.first_name,'') || ' ' || coalesce(p.last_name,'')), 'phone', p.phone,
                'cell', g.name, 'meetings', (SELECT count(*) FROM cell_meetings m WHERE m.cell_group_id = g.id AND m.date BETWEEN $2 AND $3))
         FROM member_profiles p JOIN cell_groups g ON g.id = p.cell_group_id
         WHERE p.foc_id = $1 AND p.status = 'approved' AND p.deleted_at IS NULL
           AND EXISTS (SELECT 1 FROM cell_meetings m WHERE m.cell_group_id = g.id AND m.date BETWEEN $2 AND $3)
           AND NOT EXISTS (SELECT 1 FROM cell_meetings m, jsonb_array_elements(coalesce(m.attendance, '[]'::jsonb)) x
                           WHERE m.cell_group_id = g.id AND m.date BETWEEN $2 AND $3
                             AND coalesce((CASE WHEN jsonb_typeof(x) = 'object' THEN x ->> 'present' END)::bool, true)
                             AND (CASE WHEN jsonb_typeof(x) = 'object' THEN coalesce(x ->> 'profile_id', x ->> 'member_id', x ->> 'id') ELSE x #>> '{}' END) = p.id::text)
         ORDER BY g.name, p.first_name LIMIT 100")
        .bind(foc).bind(from).bind(to).fetch_all(pool).await
}

// ───────────── visitors ─────────────

/// The visitor funnel for visitors who came in the period, and for the period before.
pub async fn visitor_funnel(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate, pfrom: NaiveDate, pto: NaiveDate) -> R<Value> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object(
            'total', count(*) FILTER (WHERE visit_date BETWEEN $2 AND $3), 'total_prev', count(*) FILTER (WHERE visit_date BETWEEN $4 AND $5),
            'new', count(*) FILTER (WHERE visit_date BETWEEN $2 AND $3 AND status = 'new'),
            'contacted', count(*) FILTER (WHERE visit_date BETWEEN $2 AND $3 AND status = 'contacted'),
            'following_up', count(*) FILTER (WHERE visit_date BETWEEN $2 AND $3 AND status = 'following_up'),
            'joined', count(*) FILTER (WHERE visit_date BETWEEN $2 AND $3 AND status = 'joined'),
            'joined_prev', count(*) FILTER (WHERE visit_date BETWEEN $4 AND $5 AND status = 'joined'),
            'closed', count(*) FILTER (WHERE visit_date BETWEEN $2 AND $3 AND status = 'closed'),
            'by_form', count(*) FILTER (WHERE visit_date BETWEEN $2 AND $3 AND source = 'form'),
            'by_admin', count(*) FILTER (WHERE visit_date BETWEEN $2 AND $3 AND source = 'admin'),
            'never_contacted', count(*) FILTER (WHERE visit_date BETWEEN $2 AND $3 AND NOT EXISTS (SELECT 1 FROM follow_ups f WHERE f.visitor_id = v.id)),
            'waiting_over_week', count(*) FILTER (WHERE status = 'new' AND visit_date < current_date - 7))
         FROM visitors v WHERE foc_id = $1")
        .bind(foc).bind(from).bind(to).bind(pfrom).bind(pto).fetch_one(pool).await
}

pub async fn visitor_sources(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object('name', coalesce(nullif(btrim(how_heard), ''), 'Not asked'), 'count', count(*), 'joined', count(*) FILTER (WHERE status = 'joined'))
         FROM visitors WHERE foc_id = $1 AND visit_date BETWEEN $2 AND $3
         GROUP BY coalesce(nullif(btrim(how_heard), ''), 'Not asked') ORDER BY count(*) DESC LIMIT 12")
        .bind(foc).bind(from).bind(to).fetch_all(pool).await
}

/// Visitors who need a call: still new after a week, or not contacted for two weeks while being followed up.
pub async fn visitors_waiting(pool: &PgPool, foc: Uuid) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object('id', v.id, 'name', v.full_name, 'phone', v.phone, 'visit_date', v.visit_date, 'status', v.status,
                'last_contact', (SELECT max(created_at)::date FROM follow_ups f WHERE f.visitor_id = v.id))
         FROM visitors v WHERE v.foc_id = $1 AND (
            (v.status = 'new' AND v.visit_date < current_date - 7) OR
            (v.status IN ('contacted','following_up') AND coalesce((SELECT max(created_at)::date FROM follow_ups f WHERE f.visitor_id = v.id), v.visit_date) < current_date - 14))
         ORDER BY v.visit_date LIMIT 100")
        .bind(foc).fetch_all(pool).await
}

pub async fn visitor_followups(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate) -> R<Value> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object('total', count(*), 'visitors', count(DISTINCT visitor_id), 'members', count(DISTINCT member_id))
         FROM follow_ups WHERE foc_id = $1 AND created_at::date BETWEEN $2 AND $3")
        .bind(foc).bind(from).bind(to).fetch_one(pool).await
}

// ───────────── community ─────────────

pub async fn prayer_stats(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate, pfrom: NaiveDate, pto: NaiveDate) -> R<Value> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object(
            'requests', count(*) FILTER (WHERE date BETWEEN $2 AND $3), 'requests_prev', count(*) FILTER (WHERE date BETWEEN $4 AND $5),
            'supports', coalesce(sum(support_count) FILTER (WHERE date BETWEEN $2 AND $3), 0),
            'supports_prev', coalesce(sum(support_count) FILTER (WHERE date BETWEEN $4 AND $5), 0),
            'anonymous', count(*) FILTER (WHERE date BETWEEN $2 AND $3 AND is_anonymous),
            'people', count(DISTINCT profile_id) FILTER (WHERE date BETWEEN $2 AND $3 AND NOT is_anonymous),
            'unsupported', count(*) FILTER (WHERE date BETWEEN $2 AND $3 AND coalesce(support_count, 0) = 0))
         FROM prayers WHERE foc_id = $1")
        .bind(foc).bind(from).bind(to).bind(pfrom).bind(pto).fetch_one(pool).await
}

/// Requests nobody has prayed for yet (anonymous ones keep their anonymity: no author is returned).
pub async fn unsupported_prayers(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object('id', id, 'title', title, 'date', date)
         FROM prayers WHERE foc_id = $1 AND date BETWEEN $2 AND $3 AND coalesce(support_count, 0) = 0 ORDER BY date DESC LIMIT 20")
        .bind(foc).bind(from).bind(to).fetch_all(pool).await
}

pub async fn testimony_stats(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate, pfrom: NaiveDate, pto: NaiveDate) -> R<Value> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object(
            'shared', count(*) FILTER (WHERE date BETWEEN $2 AND $3), 'shared_prev', count(*) FILTER (WHERE date BETWEEN $4 AND $5),
            'approved', count(*) FILTER (WHERE date BETWEEN $2 AND $3 AND status = 'approved'),
            'people', count(DISTINCT profile_id) FILTER (WHERE date BETWEEN $2 AND $3 AND NOT is_anonymous),
            'waiting', count(*) FILTER (WHERE status = 'pending'))
         FROM testimonies WHERE foc_id = $1")
        .bind(foc).bind(from).bind(to).bind(pfrom).bind(pto).fetch_one(pool).await
}

pub async fn sermon_stats(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate, pfrom: NaiveDate, pto: NaiveDate) -> R<Value> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object(
            'count', count(*) FILTER (WHERE date BETWEEN $2 AND $3), 'count_prev', count(*) FILTER (WHERE date BETWEEN $4 AND $5),
            'with_audio', count(*) FILTER (WHERE date BETWEEN $2 AND $3 AND coalesce(audio_url, '') <> ''),
            'with_video', count(*) FILTER (WHERE date BETWEEN $2 AND $3 AND coalesce(youtube_link, facebook_link, instagram_link, '') <> ''),
            'last_date', max(date), 'preachers', coalesce((SELECT jsonb_agg(jsonb_build_object('name', n, 'count', c) ORDER BY c DESC)
                FROM (SELECT coalesce(nullif(btrim(preacher), ''), 'Not recorded') n, count(*) c FROM sermons
                      WHERE foc_id = $1 AND date BETWEEN $2 AND $3 GROUP BY 1 LIMIT 8) t), '[]'::jsonb))
         FROM sermons WHERE foc_id = $1")
        .bind(foc).bind(from).bind(to).bind(pfrom).bind(pto).fetch_one(pool).await
}

pub async fn announcement_stats(pool: &PgPool, foc: Uuid) -> R<Value> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object('live', count(*) FILTER (WHERE is_active AND (end_date IS NULL OR end_date >= current_date) AND (start_date IS NULL OR start_date <= current_date)),
                'ending_soon', count(*) FILTER (WHERE is_active AND end_date BETWEEN current_date AND current_date + 7))
         FROM announcements WHERE foc_id = $1")
        .bind(foc).fetch_one(pool).await
}

/// Members who made a named prayer request or shared a named testimony in the period.
pub async fn participation(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate) -> R<Value> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object('active_members', (SELECT count(*) FROM member_profiles WHERE foc_id = $1 AND status = 'approved' AND deleted_at IS NULL),
                'taking_part', (SELECT count(DISTINCT pid) FROM (
                    SELECT profile_id AS pid FROM prayers WHERE foc_id = $1 AND date BETWEEN $2 AND $3 AND NOT is_anonymous
                    UNION ALL SELECT profile_id FROM testimonies WHERE foc_id = $1 AND date BETWEEN $2 AND $3 AND NOT is_anonymous) t))")
        .bind(foc).bind(from).bind(to).fetch_one(pool).await
}

pub async fn reports_summary(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate) -> R<Value> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object('open', count(*) FILTER (WHERE status = 'open'),
                'received', count(*) FILTER (WHERE created_at::date BETWEEN $2 AND $3),
                'handled', count(*) FILTER (WHERE handled_at::date BETWEEN $2 AND $3),
                'removed', count(*) FILTER (WHERE handled_at::date BETWEEN $2 AND $3 AND status IN ('removed','removed_and_ejected')))
         FROM content_reports WHERE foc_id = $1")
        .bind(foc).bind(from).bind(to).fetch_one(pool).await
}
