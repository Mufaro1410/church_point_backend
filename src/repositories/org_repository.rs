use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::org::{CellGroupRequest, MeetingRequest};

// Every query carries foc_id: tenant isolation lives here (Axum bypasses RLS).

pub async fn leader_in_foc(pool: &PgPool, leader: Uuid, foc_id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM member_profiles WHERE id=$1 AND foc_id=$2 AND deleted_at IS NULL)",
    )
    .bind(leader)
    .bind(foc_id)
    .fetch_one(pool)
    .await
}

pub async fn list_groups(pool: &PgPool, foc_id: Uuid) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT to_jsonb(g) || jsonb_build_object('member_count',
            (SELECT count(*) FROM member_profiles m WHERE m.cell_group_id = g.id AND m.deleted_at IS NULL))
         FROM cell_groups g WHERE g.foc_id = $1 ORDER BY g.name ASC",
    )
    .bind(foc_id)
    .fetch_all(pool)
    .await
}

pub async fn get_group(pool: &PgPool, id: Uuid, foc_id: Uuid) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar("SELECT to_jsonb(g) FROM cell_groups g WHERE g.id=$1 AND g.foc_id=$2")
        .bind(id)
        .bind(foc_id)
        .fetch_optional(pool)
        .await
}

pub async fn insert_group(pool: &PgPool, foc_id: Uuid, r: &CellGroupRequest) -> Result<Value, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO cell_groups(name, foc_id, description, location, meeting_day, start_time, end_time, leader_id)
         VALUES ($1,$2,$3,$4,$5,$6::time,$7::time,$8) RETURNING to_jsonb(cell_groups)",
    )
    .bind(r.name.as_deref())
    .bind(foc_id)
    .bind(r.description.as_deref())
    .bind(r.location.as_deref())
    .bind(r.meeting_day.as_deref())
    .bind(r.start_time.as_deref())
    .bind(r.end_time.as_deref())
    .bind(r.leader_id)
    .fetch_one(pool)
    .await
}

pub async fn update_group(
    pool: &PgPool,
    id: Uuid,
    foc_id: Uuid,
    r: &CellGroupRequest,
) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE cell_groups SET
            name = COALESCE($3, name), description = COALESCE($4, description),
            location = COALESCE($5, location), meeting_day = COALESCE($6, meeting_day),
            start_time = COALESCE($7::time, start_time), end_time = COALESCE($8::time, end_time),
            leader_id = COALESCE($9, leader_id), updated_at = now()
         WHERE id=$1 AND foc_id=$2 RETURNING to_jsonb(cell_groups)",
    )
    .bind(id)
    .bind(foc_id)
    .bind(r.name.as_deref())
    .bind(r.description.as_deref())
    .bind(r.location.as_deref())
    .bind(r.meeting_day.as_deref())
    .bind(r.start_time.as_deref())
    .bind(r.end_time.as_deref())
    .bind(r.leader_id)
    .fetch_optional(pool)
    .await
}

/// Members of the group are un-assigned (FK is ON DELETE SET NULL); meetings go with the group.
pub async fn delete_group(pool: &PgPool, id: Uuid, foc_id: Uuid) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM cell_groups WHERE id=$1 AND foc_id=$2")
        .bind(id)
        .bind(foc_id)
        .execute(pool)
        .await?
        .rows_affected())
}

pub async fn list_meetings(pool: &PgPool, group: Uuid, foc_id: Uuid) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT to_jsonb(m) FROM cell_meetings m JOIN cell_groups g ON g.id = m.cell_group_id
         WHERE m.cell_group_id=$1 AND g.foc_id=$2 ORDER BY m.date DESC",
    )
    .bind(group)
    .bind(foc_id)
    .fetch_all(pool)
    .await
}

pub async fn list_all_meetings(pool: &PgPool, foc_id: Uuid) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT to_jsonb(m) FROM cell_meetings m JOIN cell_groups g ON g.id = m.cell_group_id
         WHERE g.foc_id = $1 ORDER BY m.date DESC",
    )
    .bind(foc_id)
    .fetch_all(pool)
    .await
}

pub async fn insert_meeting(
    pool: &PgPool,
    group: Uuid,
    foc_id: Uuid,
    created_by: Uuid,
    r: &MeetingRequest,
) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO cell_meetings(cell_group_id, date, title, notes, attendance, offering, created_by)
         SELECT g.id, $3::date, $4, $5, COALESCE($6::jsonb,'[]'::jsonb), COALESCE($7::jsonb,'[]'::jsonb), $8
         FROM cell_groups g WHERE g.id=$1 AND g.foc_id=$2
         RETURNING to_jsonb(cell_meetings)",
    )
    .bind(group)
    .bind(foc_id)
    .bind(r.date.as_deref())
    .bind(r.title.as_deref())
    .bind(r.notes.as_deref())
    .bind(r.attendance.as_ref())
    .bind(r.offering.as_ref())
    .bind(created_by)
    .fetch_optional(pool)
    .await
}

pub async fn delete_meeting(pool: &PgPool, id: Uuid, foc_id: Uuid) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "DELETE FROM cell_meetings m USING cell_groups g
         WHERE m.id=$1 AND g.id = m.cell_group_id AND g.foc_id=$2",
    )
    .bind(id)
    .bind(foc_id)
    .execute(pool)
    .await?
    .rows_affected())
}

pub async fn update_meeting(pool: &PgPool, id: Uuid, foc_id: Uuid, r: &MeetingRequest) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE cell_meetings m SET date = COALESCE($3::date, m.date), title = COALESCE($4, m.title),
            notes = COALESCE($5, m.notes), attendance = COALESCE($6::jsonb, m.attendance),
            offering = COALESCE($7::jsonb, m.offering), updated_at = now()
         FROM cell_groups g WHERE m.id = $1 AND g.id = m.cell_group_id AND g.foc_id = $2
         RETURNING to_jsonb(m)",
    )
    .bind(id)
    .bind(foc_id)
    .bind(r.date.as_deref())
    .bind(r.title.as_deref())
    .bind(r.notes.as_deref())
    .bind(r.attendance.as_ref())
    .bind(r.offering.as_ref())
    .fetch_optional(pool)
    .await
}

pub async fn dashboard(pool: &PgPool, foc_id: Uuid) -> Result<Value, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object(
            'members_total',    (SELECT count(*) FROM member_profiles WHERE foc_id=$1 AND deleted_at IS NULL),
            'members_approved', (SELECT count(*) FROM member_profiles WHERE foc_id=$1 AND deleted_at IS NULL AND status='approved'),
            'members_pending',  (SELECT count(*) FROM member_profiles WHERE foc_id=$1 AND deleted_at IS NULL AND status='pending'),
            'cell_groups',      (SELECT count(*) FROM cell_groups WHERE foc_id=$1),
            'ministries',       (SELECT count(*) FROM foc_ministries WHERE foc_id=$1))",
    )
    .bind(foc_id)
    .fetch_one(pool)
    .await
}
