use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn list_focs(pool: &PgPool, status: Option<&str>) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT to_jsonb(f) - 'payment_details'
                || jsonb_build_object('member_count',
                     (SELECT count(*) FROM member_profiles m WHERE m.foc_id = f.id AND m.deleted_at IS NULL))
         FROM foc_profile f
         WHERE ($1::text IS NULL OR f.status = $1)
         ORDER BY f.name ASC",
    )
    .bind(status)
    .fetch_all(pool)
    .await
}

pub async fn set_status(pool: &PgPool, id: Uuid, status: &str) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE foc_profile SET status = $2, updated_at = now() WHERE id = $1
         RETURNING to_jsonb(foc_profile) - 'payment_details'",
    )
    .bind(id)
    .bind(status)
    .fetch_optional(pool)
    .await
}
