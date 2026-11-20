use chrono::NaiveDate;
use serde_json::Value;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::models::testimony::UpdateTestimonyRequest;

/// Anonymity is enforced here, in the query (see prayer_repository). The `author` column
/// holds a real name, so it is replaced as well as `profile_id`. $1 = caller's member id.
const MASKED: &str = "to_jsonb(t) || jsonb_build_object(
    'profile_id', CASE WHEN t.is_anonymous AND t.profile_id IS DISTINCT FROM $1 THEN NULL ELSE t.profile_id END,
    'author',     CASE WHEN t.is_anonymous AND t.profile_id IS DISTINCT FROM $1 THEN 'Anonymous'
                       WHEN t.profile_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM member_profiles mm WHERE mm.id = t.profile_id AND mm.foc_id IS NOT DISTINCT FROM t.foc_id) THEN 'Former member'
                       ELSE t.author END,
    'is_mine',    COALESCE(t.profile_id = $1, false))";

pub async fn list(
    pool: &PgPool,
    foc_id: Uuid,
    me: Uuid,
    status: Option<&str>,
    limit: i64,
) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar(&format!(
        "SELECT {MASKED} FROM testimonies t
         WHERE t.foc_id = $2 AND ($3::text IS NULL OR t.status = $3) {}
         ORDER BY t.created_at DESC LIMIT $4",
        super::moderation_repository::hidden_sql("testimony", "t")
    ))
    .bind(me)
    .bind(foc_id)
    .bind(status)
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn find(pool: &PgPool, id: Uuid, foc_id: Uuid, me: Uuid) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar(&format!(
        "SELECT {MASKED} FROM testimonies t WHERE t.id = $2 AND t.foc_id = $3"
    ))
    .bind(me)
    .bind(id)
    .bind(foc_id)
    .fetch_optional(pool)
    .await
}

pub async fn exists(pool: &PgPool, id: Uuid, foc_id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM testimonies WHERE id = $1 AND foc_id = $2)")
        .bind(id)
        .bind(foc_id)
        .fetch_one(pool)
        .await
}

#[allow(clippy::too_many_arguments)]
pub async fn insert(
    pool: &PgPool,
    foc_id: Uuid,
    profile_id: Uuid,
    author: &str,
    title: &str,
    situation: &str,
    intervention: &str,
    outcome: &str,
    is_anonymous: bool,
    date: Option<NaiveDate>,
) -> Result<Uuid, sqlx::Error> {
    // New testimonies always start 'pending'; the date falls back to the column default.
    sqlx::query_scalar(
        "INSERT INTO testimonies
           (foc_id, profile_id, author, title, situation, intervention, outcome,
            is_anonymous, status, date)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'pending', COALESCE($9::date, CURRENT_DATE))
         RETURNING id",
    )
    .bind(foc_id)
    .bind(profile_id)
    .bind(author)
    .bind(title)
    .bind(situation)
    .bind(intervention)
    .bind(outcome)
    .bind(is_anonymous)
    .bind(date)
    .fetch_one(pool)
    .await
}

/// Author-only. An edit puts the testimony back to 'pending' and clears the moderation
/// stamp, so an author cannot get approved and then rewrite it.
pub async fn update_own(
    pool: &PgPool,
    id: Uuid,
    foc_id: Uuid,
    me: Uuid,
    r: &UpdateTestimonyRequest,
) -> Result<Option<Uuid>, sqlx::Error> {
    let mut qb = QueryBuilder::<Postgres>::new(
        "UPDATE testimonies SET updated_at = now(), status = 'pending',
                moderated_by = NULL, moderated_at = NULL",
    );
    if let Some(v) = &r.title {
        qb.push(", title = ").push_bind(v);
    }
    if let Some(v) = &r.situation {
        qb.push(", situation = ").push_bind(v);
    }
    if let Some(v) = &r.intervention {
        qb.push(", intervention = ").push_bind(v);
    }
    if let Some(v) = &r.outcome {
        qb.push(", outcome = ").push_bind(v);
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

/// Idempotent: approving an approved testimony just refreshes the stamp.
/// `status` is only ever the literal "approved" or "rejected" (chosen by our code).
pub async fn moderate(
    pool: &PgPool,
    id: Uuid,
    foc_id: Uuid,
    moderator: Uuid,
    status: &'static str,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE testimonies
         SET status = $3, moderated_by = $4, moderated_at = now(), updated_at = now()
         WHERE id = $1 AND foc_id = $2 RETURNING id",
    )
    .bind(id)
    .bind(foc_id)
    .bind(status)
    .bind(moderator)
    .fetch_optional(pool)
    .await
}

pub async fn delete(
    pool: &PgPool,
    id: Uuid,
    foc_id: Uuid,
    me: Uuid,
    is_admin: bool,
) -> Result<u64, sqlx::Error> {
    let res = sqlx::query(
        "DELETE FROM testimonies WHERE id = $1 AND foc_id = $2 AND ($4 OR profile_id = $3)",
    )
    .bind(id)
    .bind(foc_id)
    .bind(me)
    .bind(is_admin)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}
