mod common;

use axum::http::StatusCode;
use common::{request, test_app};

const ID: &str = "00000000-0000-0000-0000-000000000001";

/// Every content route must reject anonymous callers, whatever the method.
#[tokio::test]
async fn every_content_route_requires_a_token() {
    let app = test_app();
    let mut routes: Vec<(&str, String)> = Vec::new();
    for base in ["sermons", "prayers", "testimonies", "announcements"] {
        routes.push(("GET", format!("/api/v1/{base}")));
        routes.push(("POST", format!("/api/v1/{base}")));
        routes.push(("GET", format!("/api/v1/{base}/{ID}")));
        routes.push(("PATCH", format!("/api/v1/{base}/{ID}")));
        routes.push(("DELETE", format!("/api/v1/{base}/{ID}")));
    }
    routes.push(("POST", format!("/api/v1/testimonies/{ID}/approve")));
    routes.push(("POST", format!("/api/v1/testimonies/{ID}/reject")));

    for (method, uri) in routes {
        let (status, body) = request(&app, method, &uri, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}");
        assert_eq!(body["error"]["code"], "MISSING_TOKEN", "{method} {uri}");
    }
}
