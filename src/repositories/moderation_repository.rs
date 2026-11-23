use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

type R<T> = Result<T, sqlx::Error>;

/// SQL that hides posts a member should not see: leaders-only posts (unless they are the author or a leader),
/// posts from people they blocked, ones they reported, and ones several people have reported that no admin has
/// looked at yet. `$1` is the viewer.
pub fn hidden_sql(kind: &str, alias: &str) -> String {
    format!(
        "AND (COALESCE({a}.is_anonymous, false) = false OR {a}.profile_id IS NOT DISTINCT FROM $1 OR EXISTS (
                SELECT 1 FROM member_profiles v WHERE v.id = $1 AND (v.role::text IN ('admin', 'super_admin') OR EXISTS (
                    SELECT 1 FROM member_church_roles mr JOIN church_roles cr ON cr.id = mr.role_id
                    WHERE mr.member_id = v.id AND cr.permissions && ARRAY['{perm}.view', '{perm}.manage', '{perm}.delete']))))
          AND ({a}.profile_id IS NOT DISTINCT FROM $1 OR (
            NOT EXISTS (SELECT 1 FROM member_blocks b WHERE b.blocker_id = $1 AND b.blocked_id = {a}.profile_id)
            AND NOT EXISTS (SELECT 1 FROM content_reports cr WHERE cr.kind = '{k}' AND cr.content_id = {a}.id AND cr.reporter_id = $1)
            AND (SELECT count(*) FROM content_reports cr WHERE cr.kind = '{k}' AND cr.content_id = {a}.id AND cr.status = 'open') < 3))",
        a = alias, k = kind, perm = if kind == "prayer" { "prayers" } else { "testimonies" }
    )
}

/// (author, anonymous, short text) of a post in this church.
pub async fn content_info(pool: &PgPool, kind: &str, id: Uuid, foc: Uuid) -> R<Option<(Option<Uuid>, bool, String)>> {
    let sql = if kind == "prayer" {
        "SELECT profile_id, is_anonymous, left(title, 200) FROM prayers WHERE id = $1 AND foc_id = $2"
    } else {
        "SELECT profile_id, COALESCE(is_anonymous, false), left(COALESCE(title, ''), 200) FROM testimonies WHERE id = $1 AND foc_id = $2"
    };
    sqlx::query_as(sql).bind(id).bind(foc).fetch_optional(pool).await
}

pub async fn insert_report(pool: &PgPool, foc: Uuid, reporter: Uuid, kind: &str, id: Uuid, author: Option<Uuid>, excerpt: &str, reason: Option<&str>) -> R<()> {
    sqlx::query(
        "INSERT INTO content_reports (foc_id, reporter_id, kind, content_id, author_id, excerpt, reason)
         VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT DO NOTHING",
    ).bind(foc).bind(reporter).bind(kind).bind(id).bind(author).bind(excerpt).bind(reason).execute(pool).await?;
    Ok(())
}

pub async fn insert_block(pool: &PgPool, blocker: Uuid, blocked: Uuid, anonymous: bool) -> R<()> {
    sqlx::query("INSERT INTO member_blocks (blocker_id, blocked_id, was_anonymous) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
        .bind(blocker).bind(blocked).bind(anonymous).execute(pool).await?;
    Ok(())
}

pub async fn blocks(pool: &PgPool, me: Uuid) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object('id', b.id, 'created_at', b.created_at, 'anonymous', b.was_anonymous,
                'name', CASE WHEN b.was_anonymous THEN NULL ELSE nullif(btrim(concat_ws(' ', m.first_name, m.last_name)), '') END)
         FROM member_blocks b JOIN member_profiles m ON m.id = b.blocked_id
         WHERE b.blocker_id = $1 ORDER BY b.created_at DESC",
    ).bind(me).fetch_all(pool).await
}

pub async fn delete_block(pool: &PgPool, id: Uuid, me: Uuid) -> R<u64> {
    Ok(sqlx::query("DELETE FROM member_blocks WHERE id = $1 AND blocker_id = $2").bind(id).bind(me).execute(pool).await?.rows_affected())
}

pub async fn under_13(pool: &PgPool, me: Uuid) -> R<bool> {
    sqlx::query_scalar("SELECT COALESCE(dob > current_date - interval '13 years', false) FROM member_profiles WHERE id = $1").bind(me).fetch_one(pool).await
}

pub async fn accept_terms(pool: &PgPool, me: Uuid, guardian: Option<&str>) -> R<()> {
    sqlx::query(
        "UPDATE member_profiles SET terms_accepted_at = COALESCE(terms_accepted_at, now()),
                guardian_name = COALESCE($2, guardian_name),
                guardian_consent_at = CASE WHEN $2 IS NOT NULL THEN now() ELSE guardian_consent_at END
         WHERE id = $1",
    ).bind(me).bind(guardian).execute(pool).await?;
    Ok(())
}

/// Open reports, one row per reported post, oldest first. Admins see the real author (needed to act on anonymous posts).
pub async fn open_reports(pool: &PgPool, foc: Uuid) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object(
            'kind', r.kind, 'content_id', r.content_id, 'reports', count(*), 'first_reported', min(r.created_at),
            'reasons', COALESCE(jsonb_agg(r.reason) FILTER (WHERE r.reason IS NOT NULL), '[]'::jsonb),
            'author_id', (array_agg(r.author_id))[1],
            'author_name', nullif(btrim(concat_ws(' ', m.first_name, m.last_name)), ''),
            'title', COALESCE(p.title, t.title, max(r.excerpt)),
            'content', COALESCE(p.content, t.situation),
            'is_anonymous', COALESCE(p.is_anonymous, t.is_anonymous, false),
            'removed', (p.id IS NULL AND t.id IS NULL))
         FROM content_reports r
         LEFT JOIN prayers p ON r.kind = 'prayer' AND p.id = r.content_id
         LEFT JOIN testimonies t ON r.kind = 'testimony' AND t.id = r.content_id
         LEFT JOIN member_profiles m ON m.id = r.author_id
         WHERE r.foc_id = $1 AND r.status = 'open'
         GROUP BY r.kind, r.content_id, p.id, t.id, m.id
         ORDER BY min(r.created_at)",
    ).bind(foc).fetch_all(pool).await
}

/// Applies the admin's decision to every open report of the post. Returns false if there was nothing open.
pub async fn act(pool: &PgPool, foc: Uuid, kind: &str, id: Uuid, action: &str, by: Uuid) -> R<bool> {
    let mut tx = pool.begin().await?;
    let author: Option<Option<Uuid>> = sqlx::query_scalar(
        "SELECT (array_agg(author_id))[1] FROM content_reports WHERE foc_id = $1 AND kind = $2 AND content_id = $3 AND status = 'open'",
    ).bind(foc).bind(kind).bind(id).fetch_optional(&mut *tx).await?;
    let Some(author) = author else { return Ok(false) };
    let status = match action {
        "dismiss" => "dismissed",
        "remove" => "removed",
        _ => "removed_and_ejected",
    };
    if action != "dismiss" {
        let table = if kind == "prayer" { "prayers" } else { "testimonies" };
        sqlx::query(&format!("DELETE FROM {table} WHERE id = $1 AND foc_id = $2")).bind(id).bind(foc).execute(&mut *tx).await?;
    }
    if action == "remove_and_eject" {
        if let Some(a) = author {
            sqlx::query("UPDATE member_profiles SET foc_id = NULL, cell_group_id = NULL, status = 'rejected', updated_at = now()
                         WHERE id = $1 AND foc_id = $2 AND deleted_at IS NULL AND role::text NOT IN ('admin', 'super_admin')")
                .bind(a).bind(foc).execute(&mut *tx).await?;
        }
    }
    let res = sqlx::query(
        "UPDATE content_reports SET status = $4, handled_at = now(), handled_by = $5
         WHERE foc_id = $1 AND kind = $2 AND content_id = $3 AND status = 'open'",
    ).bind(foc).bind(kind).bind(id).bind(status).bind(by).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(res.rows_affected() > 0)
}

pub async fn open_count(pool: &PgPool, foc: Uuid) -> R<i64> {
    sqlx::query_scalar("SELECT count(DISTINCT (kind, content_id)) FROM content_reports WHERE foc_id = $1 AND status = 'open'").bind(foc).fetch_one(pool).await
}
