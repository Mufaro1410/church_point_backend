use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

type R<T> = Result<T, sqlx::Error>;

pub struct VisitorIn {
    pub full_name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub visit_date: Option<chrono::NaiveDate>,
    pub how_heard: Option<String>,
    pub notes: Option<String>,
    pub status: Option<String>,
    pub assigned_to: Option<Uuid>,
}

const VIEW: &str = "SELECT to_jsonb(v) || jsonb_build_object(
        'assigned_name', (SELECT btrim(coalesce(m.first_name,'') || ' ' || coalesce(m.last_name,'')) FROM member_profiles m WHERE m.id = v.assigned_to),
        'follow_up_count', (SELECT count(*) FROM follow_ups f WHERE f.visitor_id = v.id),
        'last_follow_up_at', (SELECT max(f.created_at) FROM follow_ups f WHERE f.visitor_id = v.id)) FROM visitors v";

pub async fn list(pool: &PgPool, foc: Uuid, status: Option<&str>) -> R<Vec<Value>> {
    sqlx::query_scalar(&format!("{VIEW} WHERE v.foc_id = $1 AND ($2::text IS NULL OR v.status = $2) ORDER BY v.visit_date DESC, v.created_at DESC"))
        .bind(foc).bind(status).fetch_all(pool).await
}

pub async fn get(pool: &PgPool, foc: Uuid, id: Uuid) -> R<Option<Value>> {
    sqlx::query_scalar(&format!("{VIEW} WHERE v.foc_id = $1 AND v.id = $2")).bind(foc).bind(id).fetch_optional(pool).await
}

pub async fn insert(pool: &PgPool, foc: Uuid, by: Uuid, v: &VisitorIn) -> R<Uuid> {
    sqlx::query_scalar(
        "INSERT INTO visitors (foc_id, full_name, phone, email, visit_date, how_heard, notes, assigned_to, created_by)
         VALUES ($1, $2, $3, $4, COALESCE($5, current_date), $6, $7, $8, $9) RETURNING id")
        .bind(foc).bind(&v.full_name).bind(&v.phone).bind(&v.email).bind(v.visit_date)
        .bind(&v.how_heard).bind(&v.notes).bind(v.assigned_to).bind(by).fetch_one(pool).await
}

pub async fn update(pool: &PgPool, foc: Uuid, id: Uuid, v: &VisitorIn) -> R<bool> {
    Ok(sqlx::query(
        "UPDATE visitors SET full_name = COALESCE($3, full_name), phone = COALESCE($4, phone), email = COALESCE($5, email),
                visit_date = COALESCE($6, visit_date), how_heard = COALESCE($7, how_heard), notes = COALESCE($8, notes),
                status = COALESCE($9, status), assigned_to = COALESCE($10, assigned_to), updated_at = now()
         WHERE id = $2 AND foc_id = $1")
        .bind(foc).bind(id).bind(&v.full_name).bind(&v.phone).bind(&v.email).bind(v.visit_date)
        .bind(&v.how_heard).bind(&v.notes).bind(&v.status).bind(v.assigned_to).execute(pool).await?.rows_affected() > 0)
}

pub async fn remove(pool: &PgPool, foc: Uuid, id: Uuid) -> R<bool> {
    Ok(sqlx::query("DELETE FROM visitors WHERE id = $2 AND foc_id = $1").bind(foc).bind(id).execute(pool).await?.rows_affected() > 0)
}

pub async fn member_in_foc(pool: &PgPool, foc: Uuid, member: Uuid) -> R<bool> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM member_profiles WHERE id = $2 AND foc_id = $1 AND deleted_at IS NULL)")
        .bind(foc).bind(member).fetch_one(pool).await
}

pub async fn convert(pool: &PgPool, foc: Uuid, id: Uuid, member: Option<Uuid>) -> R<bool> {
    Ok(sqlx::query("UPDATE visitors SET status = 'joined', converted_member_id = COALESCE($3, converted_member_id), updated_at = now() WHERE id = $2 AND foc_id = $1")
        .bind(foc).bind(id).bind(member).execute(pool).await?.rows_affected() > 0)
}

const LOG: &str = "SELECT to_jsonb(f) || jsonb_build_object('by_name', (SELECT btrim(coalesce(m.first_name,'') || ' ' || coalesce(m.last_name,'')) FROM member_profiles m WHERE m.id = f.created_by)) FROM follow_ups f";

pub async fn log(pool: &PgPool, foc: Uuid, visitor: Option<Uuid>, member: Option<Uuid>) -> R<Vec<Value>> {
    sqlx::query_scalar(&format!("{LOG} WHERE f.foc_id = $1 AND (f.visitor_id = $2 OR f.member_id = $3) ORDER BY f.created_at DESC"))
        .bind(foc).bind(visitor).bind(member).fetch_all(pool).await
}

pub async fn log_add(pool: &PgPool, foc: Uuid, by: Uuid, visitor: Option<Uuid>, member: Option<Uuid>, kind: &str, note: Option<&str>) -> R<Value> {
    sqlx::query_scalar("INSERT INTO follow_ups (foc_id, visitor_id, member_id, kind, note, created_by) VALUES ($1,$2,$3,$4,$5,$6) RETURNING to_jsonb(follow_ups)")
        .bind(foc).bind(visitor).bind(member).bind(kind).bind(note).bind(by).fetch_one(pool).await
}

/// A logged follow-up moves a brand-new visitor on to "contacted".
pub async fn mark_contacted(pool: &PgPool, foc: Uuid, id: Uuid) -> R<()> {
    sqlx::query("UPDATE visitors SET status = 'contacted', updated_at = now() WHERE id = $2 AND foc_id = $1 AND status = 'new'")
        .bind(foc).bind(id).execute(pool).await?;
    Ok(())
}

/// Approved members whose last check-in is older than `weeks` (or who never checked in).
pub async fn absent(pool: &PgPool, foc: Uuid, weeks: i32) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object('id', m.id, 'name', btrim(coalesce(m.first_name,'') || ' ' || coalesce(m.last_name,'')),
                'phone', m.phone, 'email', m.email, 'last_attended', a.last_at,
                'last_followed_up_at', f.last_at, 'follow_up_count', coalesce(f.n, 0))
         FROM member_profiles m
         LEFT JOIN LATERAL (
             SELECT max((x.value)::timestamptz) AS last_at FROM events e, jsonb_each_text(e.attendance) x
             WHERE e.foc_id = m.foc_id AND x.key = m.id::text) a ON true
         LEFT JOIN LATERAL (
             SELECT max(created_at) AS last_at, count(*) AS n FROM follow_ups WHERE member_id = m.id) f ON true
         WHERE m.foc_id = $1 AND m.status = 'approved' AND m.deleted_at IS NULL
           AND (a.last_at IS NULL OR a.last_at < now() - make_interval(weeks => $2))
         ORDER BY a.last_at ASC NULLS FIRST, m.first_name")
        .bind(foc).bind(weeks).fetch_all(pool).await
}

// ── the public sign-in form (QR code) ──

pub async fn form_link(pool: &PgPool, foc: Uuid) -> R<Value> {
    sqlx::query_scalar(
        "WITH ins AS (INSERT INTO visitor_form_links (foc_id) VALUES ($1) ON CONFLICT (foc_id) DO NOTHING RETURNING *)
         SELECT jsonb_build_object('token', l.token, 'enabled', l.enabled)
         FROM (SELECT * FROM ins UNION ALL SELECT * FROM visitor_form_links WHERE foc_id = $1 LIMIT 1) l")
        .bind(foc).fetch_one(pool).await
}

pub async fn set_form_link(pool: &PgPool, foc: Uuid, enabled: Option<bool>, rotate: bool) -> R<()> {
    form_link(pool, foc).await?;
    sqlx::query("UPDATE visitor_form_links SET enabled = COALESCE($2, enabled),
                   token = CASE WHEN $3 THEN replace(gen_random_uuid()::text || gen_random_uuid()::text, '-', '') ELSE token END
                 WHERE foc_id = $1")
        .bind(foc).bind(enabled).bind(rotate).execute(pool).await?;
    Ok(())
}

/// The church behind an enabled form link.
pub async fn church_for_token(pool: &PgPool, token: &str) -> R<Option<(Uuid, String, Option<String>)>> {
    sqlx::query_as("SELECT f.id, f.name, f.logo FROM visitor_form_links l JOIN foc_profile f ON f.id = l.foc_id WHERE l.token = $1 AND l.enabled")
        .bind(token).fetch_optional(pool).await
}

pub async fn form_submissions_last_hour(pool: &PgPool, foc: Uuid) -> R<i64> {
    sqlx::query_scalar("SELECT count(*) FROM visitors WHERE foc_id = $1 AND source = 'form' AND created_at > now() - interval '1 hour'")
        .bind(foc).fetch_one(pool).await
}

pub async fn recent_duplicate(pool: &PgPool, foc: Uuid, phone: &str) -> R<bool> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM visitors WHERE foc_id = $1 AND phone = $2 AND created_at > now() - interval '24 hours')")
        .bind(foc).bind(phone).fetch_one(pool).await
}

pub async fn insert_from_form(pool: &PgPool, foc: Uuid, v: &VisitorIn) -> R<()> {
    sqlx::query("INSERT INTO visitors (foc_id, full_name, phone, email, how_heard, notes, source) VALUES ($1,$2,$3,$4,$5,$6,'form')")
        .bind(foc).bind(&v.full_name).bind(&v.phone).bind(&v.email).bind(&v.how_heard).bind(&v.notes).execute(pool).await?;
    Ok(())
}
