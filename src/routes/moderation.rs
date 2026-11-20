use axum::{Router, middleware::from_fn, routing::{get, post}};

use crate::{handlers::moderation as h, middleware::authorization::require_perm, state::SharedState};

/// The queue church admins work through: posts members reported. Must be actioned within 24 hours.
pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/moderation/reports", get(h::open_reports))
        .route("/moderation/reports/count", get(h::open_count))
        .route("/moderation/reports/{kind}/{id}", post(h::act))
        .route_layer(from_fn(|req, next| require_perm(&["moderation"], req, next)))
}
