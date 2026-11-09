use sqlx::PgPool;
use uuid::Uuid;

use crate::models::member::Member;

const COLUMNS: &str = "id, user_id, foc_id, email, role::text AS role, \
                       first_name, last_name, status::text AS status";

/// Every query takes `foc_id`: tenant scoping lives HERE because the Axum
/// connection bypasses Supabase RLS.
pub async fn list_by_foc(
    pool: &PgPool,
    foc_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<Member>, sqlx::Error> {
    sqlx::query_as::<_, Member>(&format!(
        "SELECT {COLUMNS} FROM member_profiles
         WHERE foc_id = $1 AND deleted_at IS NULL
         ORDER BY first_name ASC LIMIT $2 OFFSET $3"
    ))
    .bind(foc_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

pub async fn count_by_foc(pool: &PgPool, foc_id: Uuid) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(*) FROM member_profiles WHERE foc_id = $1 AND deleted_at IS NULL")
        .bind(foc_id)
        .fetch_one(pool)
        .await
}

pub async fn find_in_foc(
    pool: &PgPool,
    id: Uuid,
    foc_id: Uuid,
) -> Result<Option<Member>, sqlx::Error> {
    sqlx::query_as::<_, Member>(&format!(
        "SELECT {COLUMNS} FROM member_profiles
         WHERE id = $1 AND foc_id = $2 AND deleted_at IS NULL"
    ))
    .bind(id)
    .bind(foc_id)
    .fetch_optional(pool)
    .await
}

pub async fn update_basic(
    pool: &PgPool,
    id: Uuid,
    foc_id: Uuid,
    first_name: Option<&str>,
    last_name: Option<&str>,
) -> Result<Option<Member>, sqlx::Error> {
    sqlx::query_as::<_, Member>(&format!(
        "UPDATE member_profiles
         SET first_name = COALESCE($3, first_name),
             last_name  = COALESCE($4, last_name),
             updated_at = now()
         WHERE id = $1 AND foc_id = $2 AND deleted_at IS NULL
         RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(foc_id)
    .bind(first_name)
    .bind(last_name)
    .fetch_optional(pool)
    .await
}

// NOTE: status values are SQL literals (not bind parameters) so this works whether
// `status` is a plain text column or a Postgres enum.
pub async fn approve(pool: &PgPool, id: Uuid, foc_id: Uuid) -> Result<Option<Member>, sqlx::Error> {
    sqlx::query_as::<_, Member>(&format!(
        "UPDATE member_profiles SET status = 'approved', updated_at = now()
         WHERE id = $1 AND foc_id = $2 AND deleted_at IS NULL
         RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(foc_id)
    .fetch_optional(pool)
    .await
}

/// Rejecting = mark rejected AND release from the FOC, in ONE statement
/// (React did these as two separate calls, which could half-fail).
pub async fn reject_and_release(pool: &PgPool, id: Uuid, foc_id: Uuid) -> Result<u64, sqlx::Error> {
    let res = sqlx::query(
        "UPDATE member_profiles SET status = 'rejected', foc_id = NULL, updated_at = now()
         WHERE id = $1 AND foc_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(foc_id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

pub async fn soft_delete(pool: &PgPool, id: Uuid, foc_id: Uuid) -> Result<u64, sqlx::Error> {
    let res = sqlx::query(
        "UPDATE member_profiles SET deleted_at = now()
         WHERE id = $1 AND foc_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(foc_id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

/// Names for a set of member ids, restricted to the caller's FOC.
pub async fn names_by_ids(
    pool: &PgPool,
    ids: &[Uuid],
    foc_id: Uuid,
) -> Result<Vec<crate::models::event::MemberName>, sqlx::Error> {
    sqlx::query_as::<_, crate::models::event::MemberName>(
        "SELECT id, first_name, last_name FROM member_profiles
         WHERE id = ANY($1) AND foc_id = $2 AND deleted_at IS NULL",
    )
    .bind(ids)
    .bind(foc_id)
    .fetch_all(pool)
    .await
}
