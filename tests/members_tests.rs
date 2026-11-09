mod common;

use axum::http::StatusCode;
use common::{request, test_app};

const ID: &str = "00000000-0000-0000-0000-000000000001";

#[tokio::test]
async fn every_member_and_foc_route_requires_a_token() {
    let app = test_app();
    let routes = [
        ("GET", "/api/v1/members".to_string()),
        ("GET", format!("/api/v1/members/{ID}")),
        ("PATCH", format!("/api/v1/members/{ID}")),
        ("DELETE", format!("/api/v1/members/{ID}")),
        ("POST", format!("/api/v1/members/{ID}/approve")),
        ("POST", format!("/api/v1/members/{ID}/reject")),
        ("GET", "/api/v1/foc".to_string()),
        ("GET", "/api/v1/provinces".to_string()),
    ];
    for (method, uri) in routes {
        let (status, body) = request(&app, method, &uri, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}");
        assert_eq!(body["error"]["code"], "MISSING_TOKEN", "{method} {uri}");
    }
}
