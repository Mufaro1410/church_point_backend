mod common;

use axum::http::StatusCode;
use common::{get, test_app};

#[tokio::test]
async fn health_is_public() {
    let (status, body) = get(&test_app(), "/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn protected_route_requires_token() {
    let (status, body) = get(&test_app(), "/api/v1/auth/me", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "MISSING_TOKEN");
}

#[tokio::test]
async fn garbage_token_rejected() {
    let (status, body) = get(&test_app(), "/api/v1/members", Some("not-a-jwt")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "INVALID_TOKEN");
}
