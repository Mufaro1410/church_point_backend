use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

type R<T> = Result<T, sqlx::Error>;

pub async fn list(pool: &PgPool, foc: Uuid) -> R<Vec<Value>> {
    sqlx::query_scalar("SELECT to_jsonb(c) FROM serving_capacities c WHERE c.foc_id = $1 ORDER BY lower(c.name)")
        .bind(foc).fetch_all(pool).await
}

pub async fn add(pool: &PgPool, foc: Uuid, name: &str) -> R<Value> {
    sqlx::query_scalar("INSERT INTO serving_capacities (foc_id, name) VALUES ($1, $2) RETURNING to_jsonb(serving_capacities)")
        .bind(foc).bind(name).fetch_one(pool).await
}

pub async fn rename(pool: &PgPool, foc: Uuid, id: Uuid, name: &str) -> R<Option<Value>> {
    sqlx::query_scalar("UPDATE serving_capacities SET name = $3, updated_at = now() WHERE id = $2 AND foc_id = $1 RETURNING to_jsonb(serving_capacities)")
        .bind(foc).bind(id).bind(name).fetch_optional(pool).await
}

pub async fn remove(pool: &PgPool, foc: Uuid, id: Uuid) -> R<bool> {
    Ok(sqlx::query("DELETE FROM serving_capacities WHERE id = $2 AND foc_id = $1").bind(foc).bind(id).execute(pool).await?.rows_affected() > 0)
}
