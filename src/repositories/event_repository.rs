use chrono::{DateTime, NaiveDate, Utc};
use serde_json::Value;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use super::scope_sql::push_manageable;
use crate::{
    middleware::authentication::AuthUser,
    models::event::{NewEvent, UpdateEventRequest},
};

// Reads return `to_jsonb(row)`, so responses keep exactly the shape Supabase gave React
// (e.g. start_time as "09:00:00+00"). Writes are strictly typed (see models/event.rs).

/// Which events the caller's tenant may see/change (mirrors `get_visible_events`).
/// Arguments are the positions of the church / province / foc bind parameters.
fn visible(church: u8, province: u8, foc: u8) -> String {
    format!(
        "((events.scope::text = 'church'   AND events.church_id   = ${church}) \
          OR (events.scope::text = 'province' AND events.province_id = ${province}) \
          OR (events.scope::text = 'foc'      AND events.foc_id      = ${foc}))"
    )
}

pub async fn list_visible(
    pool: &PgPool,
    foc_id: Uuid,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar("SELECT to_jsonb(e) FROM get_visible_events($1, $2::date, $3::date) e")
        .bind(foc_id)
        .bind(start)
        .bind(end)
        .fetch_all(pool)
        .await
}

pub async fn find_visible(pool: &PgPool, id: Uuid, auth: &AuthUser) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar(&format!(
        "SELECT to_jsonb(events) FROM events WHERE events.id = $1 AND {}",
        visible(2, 3, 4)
    ))
        .bind(id)
        .bind(auth.church_id)
        .bind(auth.province_id)
        .bind(auth.foc_id)
        .fetch_optional(pool)
        .await
}

pub async fn insert(pool: &PgPool, e: &NewEvent) -> Result<Value, sqlx::Error> {
    // start_time/end_time are `timetz`; we bind text and let Postgres parse it.
    sqlx::query_scalar(
        r#"INSERT INTO events
             (id, title, date, start_time, end_time, location, description, scope,
              church_id, province_id, foc_id, created_by_foc_id, "isRecurring", sermon_id,
              manual_stats, qr_code, qr_generated_at)
           VALUES ($1, $2, $3, $4::timetz, $5::timetz, $6, $7, $8,
                   $9, $10, $11, $12, $13, $14, $15, $16, $17)
           RETURNING to_jsonb(events)"#,
    )
    .bind(e.id)
    .bind(&e.title)
    .bind(e.date)
    .bind(&e.start_time)
    .bind(&e.end_time)
    .bind(&e.location)
    .bind(&e.description)
    .bind(e.scope)
    .bind(e.church_id)
    .bind(e.province_id)
    .bind(e.foc_id)
    .bind(e.created_by_foc_id)
    .bind(&e.is_recurring)
    .bind(e.sermon_id)
    .bind(&e.manual_stats)
    .bind(&e.qr_code)
    .bind(e.qr_generated_at)
    .fetch_one(pool)
    .await
}

/// Builds `UPDATE ... SET` from only the fields the client actually sent.
/// Every value is a bind parameter (no string-built SQL from user input).
pub async fn update(
    pool: &PgPool,
    id: Uuid,
    auth: &AuthUser,
    req: &UpdateEventRequest,
    owner: Option<&crate::services::scope::Ownership>,
) -> Result<Option<Value>, sqlx::Error> {
    let mut qb = QueryBuilder::<Postgres>::new("UPDATE events SET updated_at = now()");

    if let Some(o) = owner {
        qb.push(", scope = ").push_bind(o.scope);
        qb.push(", church_id = ").push_bind(o.church_id);
        qb.push(", province_id = ").push_bind(o.province_id);
        qb.push(", foc_id = ").push_bind(o.foc_id);
    }

    if let Some(v) = &req.title {
        qb.push(", title = ").push_bind(v);
    }
    if let Some(v) = req.date {
        qb.push(", date = ").push_bind(v);
    }
    if let Some(v) = &req.start_time {
        qb.push(", start_time = ").push_bind(v).push("::timetz");
    }
    if let Some(v) = &req.end_time {
        qb.push(", end_time = ").push_bind(v).push("::timetz");
    }
    if let Some(v) = &req.location {
        qb.push(", location = ").push_bind(v);
    }
    if let Some(v) = &req.description {
        qb.push(", description = ").push_bind(v.clone());
    }
    if let Some(v) = &req.is_recurring {
        qb.push(", \"isRecurring\" = ").push_bind(v.clone());
    }
    if let Some(v) = &req.sermon_id {
        qb.push(", sermon_id = ").push_bind(*v);
    }
    if let Some(v) = &req.manual_stats {
        // `||` merges keys: only the counters sent are overwritten.
        qb.push(", manual_stats = COALESCE(manual_stats, '{}'::jsonb) || ")
            .push_bind(serde_json::to_value(v).unwrap_or(Value::Null));
    }

    qb.push(" WHERE events.id = ").push_bind(id).push(" AND ");
    push_manageable(&mut qb, "events", auth);
    qb.push(" RETURNING to_jsonb(events)");

    qb.build_query_scalar::<Value>().fetch_optional(pool).await
}

pub async fn set_qr(
    pool: &PgPool,
    id: Uuid,
    auth: &AuthUser,
    qr_code: &str,
    at: DateTime<Utc>,
) -> Result<Option<Value>, sqlx::Error> {
    let mut qb = QueryBuilder::<Postgres>::new("UPDATE events SET qr_code = ");
    qb.push_bind(qr_code).push(", qr_generated_at = ").push_bind(at);
    qb.push(", updated_at = now() WHERE events.id = ").push_bind(id).push(" AND ");
    push_manageable(&mut qb, "events", auth);
    qb.push(" RETURNING to_jsonb(events)");
    qb.build_query_scalar::<Value>().fetch_optional(pool).await
}

/// Integrity rule: an event may only be linked to a sermon of the caller's own FOC.
pub async fn sermon_in_foc(pool: &PgPool, sermon_id: Uuid, foc_id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM sermons WHERE id = $1 AND foc_id = $2)")
        .bind(sermon_id)
        .bind(foc_id)
        .fetch_one(pool)
        .await
}

pub async fn delete(pool: &PgPool, id: Uuid, auth: &AuthUser) -> Result<u64, sqlx::Error> {
    let mut qb = QueryBuilder::<Postgres>::new("DELETE FROM events WHERE events.id = ");
    qb.push_bind(id).push(" AND ");
    push_manageable(&mut qb, "events", auth);
    Ok(qb.build().execute(pool).await?.rows_affected())
}

/// Atomic check-in: sets attendance[member_id] = now(), keeping the original time if the
/// member was already checked in. One statement = no lost updates when two scanners
/// check in different people at the same moment.
pub async fn mark_attendance(
    pool: &PgPool,
    id: Uuid,
    member_id: Uuid,
    auth: &AuthUser,
) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar(&format!(
        "UPDATE events SET attendance = CASE
             WHEN COALESCE(attendance, '{{}}'::jsonb) ? $2::text THEN attendance
             ELSE jsonb_set(COALESCE(attendance, '{{}}'::jsonb), ARRAY[$2::text],
                  to_jsonb(to_char(now() AT TIME ZONE 'utc', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')), true)
           END
         WHERE events.id = $1 AND {}
         RETURNING to_jsonb(events)",
        visible(3, 4, 5)
    ))
    .bind(id)
    .bind(member_id.to_string())
    .bind(auth.church_id)
    .bind(auth.province_id)
    .bind(auth.foc_id)
    .fetch_optional(pool)
    .await
}
