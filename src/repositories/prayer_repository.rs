use chrono::NaiveDate;
use serde_json::Value;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::models::prayer::UpdatePrayerRequest;

/// ANONYMITY IS ENFORCED HERE, in the query, so the author's id never leaves the database
/// layer for anyone but the author. Also resolves the display name server-side
/// (replaces the React `resolveDisplayName` helper).
/// $1 = the caller's member id.
const MASKED: &str = "to_jsonb(p) || jsonb_build_object(
    'profile_id', CASE WHEN p.is_anonymous AND p.profile_id <> $1 THEN NULL ELSE p.profile_id END,
    'author_name', CASE WHEN p.is_anonymous THEN 'Anonymous'
                        WHEN m.foc_id IS DISTINCT FROM p.foc_id THEN 'Former member'
                        ELSE NULLIF(btrim(concat_ws(' ', m.first_name, m.last_name)), '') END,
    'is_mine', (p.profile_id = $1))";

pub async fn list(
    pool: &PgPool,
    foc_id: Uuid,
    me: Uuid,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar(&format!(
        "SELECT {MASKED} FROM prayers p LEFT JOIN member_profiles m ON m.id = p.profile_id
         WHERE p.foc_id = $2 AND p.date BETWEEN $3 AND $4
         ORDER BY p.created_at DESC"
    ))
    .bind(me)
    .bind(foc_id)
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await
}

pub async fn find(pool: &PgPool, id: Uuid, foc_id: Uuid, me: Uuid) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar(&format!(
        "SELECT {MASKED} FROM prayers p LEFT JOIN member_profiles m ON m.id = p.profile_id
         WHERE p.id = $2 AND p.foc_id = $3"
    ))
    .bind(me)
    .bind(id)
    .bind(foc_id)
    .fetch_optional(pool)
    .await
}

/// True if the prayer exists in this FOC (used to tell "not yours" from "not found").
pub async fn exists(pool: &PgPool, id: Uuid, foc_id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM prayers WHERE id = $1 AND foc_id = $2)")
        .bind(id)
        .bind(foc_id)
        .fetch_one(pool)
        .await
}

pub async fn insert(
    pool: &PgPool,
    foc_id: Uuid,
    profile_id: Uuid,
    title: &str,
    content: &str,
    is_anonymous: bool,
    date: NaiveDate,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO prayers (foc_id, profile_id, title, content, is_anonymous, date, support_count)
         VALUES ($1, $2, $3, $4, $5, $6, 0) RETURNING id",
    )
    .bind(foc_id)
    .bind(profile_id)
    .bind(title)
    .bind(content)
    .bind(is_anonymous)
    .bind(date)
    .fetch_one(pool)
    .await
}

/// Author-only: the WHERE clause includes `profile_id = me`.
pub async fn update_own(
    pool: &PgPool,
    id: Uuid,
    foc_id: Uuid,
    me: Uuid,
    r: &UpdatePrayerRequest,
) -> Result<Option<Uuid>, sqlx::Error> {
    let mut qb = QueryBuilder::<Postgres>::new("UPDATE prayers SET updated_at = now()");
    if let Some(v) = &r.title {
        qb.push(", title = ").push_bind(v);
    }
    if let Some(v) = &r.content {
        qb.push(", content = ").push_bind(v);
    }
    if let Some(v) = r.is_anonymous {
        qb.push(", is_anonymous = ").push_bind(v);
    }
    qb.push(" WHERE id = ").push_bind(id);
    qb.push(" AND foc_id = ").push_bind(foc_id);
    qb.push(" AND profile_id = ").push_bind(me);
    qb.push(" RETURNING id");
    qb.build_query_scalar::<Uuid>().fetch_optional(pool).await
}

/// The author, or an admin moderating.
pub async fn delete(
    pool: &PgPool,
    id: Uuid,
    foc_id: Uuid,
    me: Uuid,
    is_admin: bool,
) -> Result<u64, sqlx::Error> {
    let res = sqlx::query(
        "DELETE FROM prayers WHERE id = $1 AND foc_id = $2 AND ($4 OR profile_id = $3)",
    )
    .bind(id)
    .bind(foc_id)
    .bind(me)
    .bind(is_admin)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}
