//! Idempotency for write requests.
//!
//! A client may send `Idempotency-Key: <random text>` with a POST/PUT/PATCH/DELETE.
//! The first request with that key runs normally and its (successful) answer is stored.
//! The same key again returns the stored answer instead of doing the work twice.
//! While the first is still running, a second one gets 409 so nothing is done in parallel.
//! Failed attempts are forgotten, so the person can fix the problem and try again.

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use uuid::Uuid;

use crate::{errors::AppError, middleware::authentication::AuthUser, state::SharedState};

const MAX_STORED_BYTES: usize = 2 * 1024 * 1024;

enum Claim {
    Fresh,
    Replay(i32, serde_json::Value),
    InProgress,
    DifferentRequest,
}

async fn claim(state: &SharedState, user: Uuid, key: &str, method: &str, path: &str) -> Result<Claim, sqlx::Error> {
    let inserted = sqlx::query(
        "INSERT INTO idempotency_keys (user_id, key, method, path) VALUES ($1,$2,$3,$4) ON CONFLICT DO NOTHING",
    )
    .bind(user).bind(key).bind(method).bind(path)
    .execute(&state.db)
    .await?
    .rows_affected();
    if inserted == 1 {
        return Ok(Claim::Fresh);
    }
    let row: Option<(String, String, Option<i32>, Option<serde_json::Value>)> = sqlx::query_as(
        "SELECT method, path, status_code, response FROM idempotency_keys WHERE user_id = $1 AND key = $2",
    )
    .bind(user).bind(key)
    .fetch_optional(&state.db)
    .await?;
    Ok(match row {
        None => Claim::Fresh, // removed in between — treat as new
        Some((m, p, _, _)) if m != method || p != path => Claim::DifferentRequest,
        Some((_, _, Some(code), Some(body))) => Claim::Replay(code, body),
        Some(_) => Claim::InProgress,
    })
}

async fn forget(state: &SharedState, user: Uuid, key: &str) {
    let _ = sqlx::query("DELETE FROM idempotency_keys WHERE user_id = $1 AND key = $2")
        .bind(user).bind(key).execute(&state.db).await;
}

pub async fn idempotent(State(state): State<SharedState>, req: Request, next: Next) -> Response {
    let mutating = matches!(*req.method(), Method::POST | Method::PUT | Method::PATCH | Method::DELETE);
    let key = req
        .headers()
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|k| !k.is_empty() && k.len() <= 100)
        .map(str::to_string);
    let (Some(key), true) = (key, mutating) else { return next.run(req).await };
    let Some(user) = req.extensions().get::<AuthUser>().map(|a| a.user_id) else { return next.run(req).await };
    let (method, path) = (req.method().as_str().to_string(), req.uri().path().to_string());

    match claim(&state, user, &key, &method, &path).await {
        Ok(Claim::Fresh) => {}
        Ok(Claim::Replay(code, body)) => {
            let status = StatusCode::from_u16(code as u16).unwrap_or(StatusCode::OK);
            let mut res = (status, axum::Json(body)).into_response();
            res.headers_mut().insert("idempotent-replay", HeaderValue::from_static("true"));
            return res;
        }
        Ok(Claim::InProgress) => {
            return AppError::Conflict("This request is already being processed. Please wait a moment.".into()).into_response();
        }
        Ok(Claim::DifferentRequest) => {
            return AppError::Validation("This request key was already used for a different request.".into()).into_response();
        }
        Err(e) => {
            // Never block real work because the safety net is unavailable.
            tracing::warn!(error = ?e, "idempotency check failed; continuing without it");
            return next.run(req).await;
        }
    }

    let res = next.run(req).await;
    let (parts, body) = res.into_parts();
    let bytes = match axum::body::to_bytes(body, MAX_STORED_BYTES).await {
        Ok(b) => b,
        Err(_) => {
            forget(&state, user, &key).await;
            return AppError::Internal("response too large to store".into()).into_response();
        }
    };

    if parts.status.is_success() {
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        let _ = sqlx::query("UPDATE idempotency_keys SET status_code = $3, response = $4 WHERE user_id = $1 AND key = $2")
            .bind(user).bind(&key).bind(parts.status.as_u16() as i32).bind(json)
            .execute(&state.db)
            .await;
    } else {
        forget(&state, user, &key).await;
    }
    let mut res = Response::from_parts(parts, Body::from(bytes));
    res.headers_mut().remove(header::CONTENT_LENGTH);
    res
}
