use sqlx::PgPool;
use uuid::Uuid;

use crate::models::auth::AuthProfile;

/// Maps a Supabase auth user id to their member profile, FOC and church.
pub async fn find_auth_profile(
    pool: &PgPool,
    auth_user_id: Uuid,
) -> Result<Option<AuthProfile>, sqlx::Error> {
    sqlx::query_as::<_, AuthProfile>(
        "SELECT m.id AS member_id, m.foc_id, f.church_id, f.province_id, m.role::text AS role
         FROM member_profiles m
         LEFT JOIN foc_profile f ON f.id = m.foc_id
         WHERE m.user_id = $1 AND m.deleted_at IS NULL",
    )
    .bind(auth_user_id)
    .fetch_optional(pool)
    .await
}
