use axum::{Json, extract::State};
use serde_json::{Value, json};

use crate::{errors::AppResult, state::SharedState};

pub async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

/// Readiness: can we reach the database?
pub async fn ready(State(state): State<SharedState>) -> AppResult<Json<Value>> {
    sqlx::query("SELECT 1").execute(&state.db).await?;
    Ok(Json(json!({ "status": "ready" })))
}
