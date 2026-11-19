use sqlx::PgPool;
use uuid::Uuid;

use crate::models::auth::AuthProfile;

/// The caller's own full profile row (works even before a FOC is approved).
pub async fn own_profile(pool: &PgPool, member_id: Uuid) -> Result<Option<serde_json::Value>, sqlx::Error> {
    sqlx::query_scalar("SELECT to_jsonb(m) FROM member_profiles m WHERE m.id = $1 AND m.deleted_at IS NULL")
        .bind(member_id)
        .fetch_optional(pool)
        .await
}

/// Maps a Supabase auth user id to their member profile, FOC and church.
pub async fn find_auth_profile(
    pool: &PgPool,
    auth_user_id: Uuid,
) -> Result<Option<AuthProfile>, sqlx::Error> {
    sqlx::query_as::<_, AuthProfile>(
        "SELECT m.id AS member_id, m.foc_id, f.church_id, f.province_id, f.status AS foc_status, m.role::text AS role, m.status::text AS member_status,
                COALESCE((SELECT array_agg(DISTINCT p) FROM member_church_roles mr JOIN church_roles r ON r.id = mr.role_id AND r.foc_id = m.foc_id, unnest(r.permissions) p WHERE mr.member_id = m.id), '{}') AS permissions
         FROM member_profiles m
         LEFT JOIN foc_profile f ON f.id = m.foc_id
         WHERE m.user_id = $1 AND m.deleted_at IS NULL",
    )
    .bind(auth_user_id)
    .fetch_optional(pool)
    .await
}
