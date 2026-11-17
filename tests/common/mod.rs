#![allow(dead_code)] // each test file compiles this module separately

pub mod db;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use churchpoint_backend::{build_app, config::Settings, state::AppState};
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

pub fn test_app() -> Router {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://u:p@127.0.0.1:1/none")
        .expect("lazy pool");
    let settings = Settings {
        host: "127.0.0.1".into(),
        port: 0,
        database_url: String::new(),
        db_max_connections: 1,
        db_min_connections: 0,
        auth_cache_secs: 0,
        ffmpeg_bin: "ffmpeg".into(),
        audio_max_mb: 5,
        supabase_jwt_secret: "test-secret".into(),
        supabase_url: None,
        supabase_anon_key: None,
        supabase_service_key: None,
        cors_origins: vec![],
    };
    build_app(AppState::new(pool, settings))
}

pub async fn get(app: &Router, uri: &str, token: Option<&str>) -> (StatusCode, Value) {
    request(app, "GET", uri, token).await
}

pub async fn request(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(uri);
    if let Some(t) = token {
        req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    let res = app.clone().oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}
