use axum::{Router, middleware::from_fn, routing::get};

use crate::{handlers::report as h, middleware::authorization::require_perm, state::SharedState};

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/reports/overview", get(h::overview))
        .route("/reports/attendance", get(h::attendance))
        .route("/reports/giving", get(h::giving))
        .route("/reports/members", get(h::members))
        .route("/reports/cells", get(h::cells))
        .route("/reports/attention", get(h::attention))
        .route("/reports/community", get(h::community))
        .route("/reports/visitors", get(h::visitors))
        .route_layer(from_fn(|req, next| require_perm(&["reports"], req, next)))
}
