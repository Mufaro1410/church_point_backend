use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

// Reference tables are returned as raw JSON via Postgres `to_jsonb(row)`.
// This is a deliberate migration shortcut: the response has exactly the same shape
// Supabase returned to React (`select('*')`), so the frontend needs no changes.
// We can replace these with typed structs later, one table at a time.

pub async fn find_by_id(pool: &PgPool, foc_id: Uuid) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar("SELECT to_jsonb(f) FROM foc_profile f WHERE f.id = $1")
        .bind(foc_id)
        .fetch_optional(pool)
        .await
}

pub async fn list_provinces(pool: &PgPool, church_id: Uuid) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar("SELECT to_jsonb(p) FROM province p WHERE p.church_id = $1 ORDER BY p.name ASC")
        .bind(church_id)
        .fetch_all(pool)
        .await
}
