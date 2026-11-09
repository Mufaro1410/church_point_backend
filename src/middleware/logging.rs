use axum::{body::Body, http::Request};
use tracing::{Span, info_span};

use super::request_id::REQUEST_ID_HEADER;

pub fn make_span(req: &Request<Body>) -> Span {
    let request_id = req.headers().get(&REQUEST_ID_HEADER).and_then(|v| v.to_str().ok()).unwrap_or("-");
    info_span!("http", method = %req.method(), uri = %req.uri().path(), %request_id)
}
