mod common;

use axum::http::StatusCode;
use common::{request, test_app};

const ID: &str = "00000000-0000-0000-0000-000000000001";

#[tokio::test]
async fn every_event_route_requires_a_token() {
    let app = test_app();
    let routes = [
        ("GET", "/api/v1/events".to_string()),
        ("POST", "/api/v1/events".to_string()),
        ("GET", format!("/api/v1/events/{ID}")),
        ("PATCH", format!("/api/v1/events/{ID}")),
        ("DELETE", format!("/api/v1/events/{ID}")),
        ("GET", format!("/api/v1/events/{ID}/rsvp")),
        ("GET", format!("/api/v1/events/{ID}/attendance")),
        ("POST", format!("/api/v1/events/{ID}/attendance")),
        ("POST", format!("/api/v1/events/{ID}/qr")),
    ];
    for (method, uri) in routes {
        let (status, body) = request(&app, method, &uri, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}");
        assert_eq!(body["error"]["code"], "MISSING_TOKEN", "{method} {uri}");
    }
}
