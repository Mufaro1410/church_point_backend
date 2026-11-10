use chrono::NaiveDate;
use serde_json::Value;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::models::sermon::{NewSermon, UpdateSermonRequest};

/// Every query is scoped by `foc_id`: the Axum connection bypasses Supabase RLS,
/// so tenant isolation lives here.

pub async fn list(
    pool: &PgPool,
    foc_id: Uuid,
    start: NaiveDate,
    end: NaiveDate,
    full: bool,
) -> Result<Vec<Value>, sqlx::Error> {
    // `- 'content'` drops the large text from the JSON unless the caller asked for it.
    let expr = if full { "to_jsonb(s)" } else { "to_jsonb(s) - 'content'" };
    sqlx::query_scalar(&format!(
        "SELECT {expr} FROM sermons s
         WHERE s.foc_id = $1 AND s.date BETWEEN $2 AND $3
         ORDER BY s.date DESC, s.created_at DESC"
    ))
    .bind(foc_id)
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await
}

pub async fn find(pool: &PgPool, id: Uuid, foc_id: Uuid) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar("SELECT to_jsonb(s) FROM sermons s WHERE s.id = $1 AND s.foc_id = $2")
        .bind(id)
        .bind(foc_id)
        .fetch_optional(pool)
        .await
}

pub async fn insert(pool: &PgPool, s: &NewSermon) -> Result<Value, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO sermons
           (foc_id, profile_id, title, preacher, date, content, scripture, summary, tags,
            audio_url, youtube_link, facebook_link, instagram_link, twitter_link, website_link)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)
         RETURNING to_jsonb(sermons)",
    )
    .bind(s.foc_id)
    .bind(s.profile_id)
    .bind(&s.title)
    .bind(&s.preacher)
    .bind(s.date)
    .bind(&s.content)
    .bind(&s.scripture)
    .bind(&s.summary)
    .bind(&s.tags)
    .bind(&s.audio_url)
    .bind(&s.youtube_link)
    .bind(&s.facebook_link)
    .bind(&s.instagram_link)
    .bind(&s.twitter_link)
    .bind(&s.website_link)
    .fetch_one(pool)
    .await
}

/// Sets `col = value` only when the client sent the field. Works for plain
/// (`Option<T>`) and nullable (`Option<Option<T>>`) fields: an inner None binds NULL.
macro_rules! set {
    ($qb:ident, $col:literal, $val:expr) => {
        if let Some(v) = $val {
            $qb.push(concat!(", ", $col, " = ")).push_bind(v.clone());
        }
    };
}

pub async fn update(
    pool: &PgPool,
    id: Uuid,
    foc_id: Uuid,
    r: &UpdateSermonRequest,
) -> Result<Option<Value>, sqlx::Error> {
    let mut qb = QueryBuilder::<Postgres>::new("UPDATE sermons SET updated_at = now()");
    set!(qb, "title", &r.title);
    set!(qb, "preacher", &r.preacher);
    set!(qb, "date", &r.date);
    set!(qb, "content", &r.content);
    set!(qb, "scripture", &r.scripture);
    set!(qb, "summary", &r.summary);
    set!(qb, "tags", &r.tags);
    set!(qb, "audio_url", &r.audio_url);
    set!(qb, "youtube_link", &r.youtube_link);
    set!(qb, "facebook_link", &r.facebook_link);
    set!(qb, "instagram_link", &r.instagram_link);
    set!(qb, "twitter_link", &r.twitter_link);
    set!(qb, "website_link", &r.website_link);
    qb.push(" WHERE sermons.id = ").push_bind(id);
    qb.push(" AND sermons.foc_id = ").push_bind(foc_id);
    qb.push(" RETURNING to_jsonb(sermons)");
    qb.build_query_scalar::<Value>().fetch_optional(pool).await
}

pub async fn delete(pool: &PgPool, id: Uuid, foc_id: Uuid) -> Result<u64, sqlx::Error> {
    let res = sqlx::query("DELETE FROM sermons WHERE id = $1 AND foc_id = $2")
        .bind(id)
        .bind(foc_id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected())
}
