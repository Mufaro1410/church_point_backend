use chrono::NaiveDate;
use serde_json::Value;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use super::scope_sql::{push_manageable, push_visible};
use crate::{
    middleware::authentication::AuthUser,
    models::announcement::{NewAnnouncement, UpdateAnnouncementRequest},
};

/// Replaces the React `.or('foc_id.eq...,and(scope.eq.church...)')` filter query.
pub async fn list_visible(pool: &PgPool, auth: &AuthUser, active_only: bool) -> Result<Vec<Value>, sqlx::Error> {
    let mut qb = QueryBuilder::<Postgres>::new("SELECT to_jsonb(announcements) FROM announcements WHERE ");
    push_visible(&mut qb, "announcements", auth);
    if active_only {
        qb.push(
            " AND announcements.is_active IS NOT FALSE
              AND announcements.start_date <= CURRENT_DATE
              AND (announcements.end_date IS NULL OR announcements.end_date >= CURRENT_DATE)",
        );
    }
    qb.push(" ORDER BY announcements.created_at DESC LIMIT 300");
    qb.build_query_scalar::<Value>().fetch_all(pool).await
}

pub async fn find_visible(pool: &PgPool, id: Uuid, auth: &AuthUser) -> Result<Option<Value>, sqlx::Error> {
    let mut qb = QueryBuilder::<Postgres>::new("SELECT to_jsonb(announcements) FROM announcements WHERE announcements.id = ");
    qb.push_bind(id).push(" AND ");
    push_visible(&mut qb, "announcements", auth);
    qb.build_query_scalar::<Value>().fetch_optional(pool).await
}

pub async fn insert(pool: &PgPool, a: &NewAnnouncement) -> Result<Value, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO announcements
           (title, content, scope, church_id, province_id, foc_id, created_by_foc_id, created_by,
            priority, category, start_date, end_date, is_active)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
         RETURNING to_jsonb(announcements)",
    )
    .bind(&a.title)
    .bind(&a.content)
    .bind(a.scope)
    .bind(a.church_id)
    .bind(a.province_id)
    .bind(a.foc_id)
    .bind(a.created_by_foc_id)
    .bind(a.created_by)
    .bind(a.priority)
    .bind(&a.category)
    .bind(a.start_date)
    .bind(a.end_date)
    .bind(a.is_active)
    .fetch_one(pool)
    .await
}

pub async fn update(
    pool: &PgPool,
    id: Uuid,
    auth: &AuthUser,
    r: &UpdateAnnouncementRequest,
) -> Result<Option<Value>, sqlx::Error> {
    let mut qb = QueryBuilder::<Postgres>::new("UPDATE announcements SET updated_at = now()");
    if let Some(v) = &r.title {
        qb.push(", title = ").push_bind(v);
    }
    if let Some(v) = &r.content {
        qb.push(", content = ").push_bind(v);
    }
    if let Some(v) = r.priority {
        qb.push(", priority = ").push_bind(v.as_str());
    }
    if let Some(v) = &r.category {
        qb.push(", category = ").push_bind(v.clone());
    }
    if let Some(v) = r.start_date {
        qb.push(", start_date = ").push_bind(v);
    }
    if let Some(v) = r.end_date {
        qb.push(", end_date = ").push_bind(v);
    }
    if let Some(v) = r.is_active {
        qb.push(", is_active = ").push_bind(v);
    }
    qb.push(" WHERE announcements.id = ").push_bind(id).push(" AND ");
    push_manageable(&mut qb, "announcements", auth);
    qb.push(" RETURNING to_jsonb(announcements)");
    qb.build_query_scalar::<Value>().fetch_optional(pool).await
}

pub async fn delete(pool: &PgPool, id: Uuid, auth: &AuthUser) -> Result<u64, sqlx::Error> {
    let mut qb = QueryBuilder::<Postgres>::new("DELETE FROM announcements WHERE announcements.id = ");
    qb.push_bind(id).push(" AND ");
    push_manageable(&mut qb, "announcements", auth);
    Ok(qb.build().execute(pool).await?.rows_affected())
}

/// For validating `end_date >= start_date` on partial updates.
pub async fn dates(pool: &PgPool, id: Uuid, auth: &AuthUser) -> Result<Option<(NaiveDate, Option<NaiveDate>)>, sqlx::Error> {
    let mut qb = QueryBuilder::<Postgres>::new("SELECT start_date, end_date FROM announcements WHERE announcements.id = ");
    qb.push_bind(id).push(" AND ");
    push_visible(&mut qb, "announcements", auth);
    qb.build_query_as::<(NaiveDate, Option<NaiveDate>)>().fetch_optional(pool).await
}
