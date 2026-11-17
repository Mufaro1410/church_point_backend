use axum::{Router, middleware::from_fn, routing::{get, put}};

use crate::{
    handlers::platform,
    middleware::authorization::{SUPER_ADMIN, require_roles},
    state::SharedState,
};

/// Platform owner only.
pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/platform/focs", get(platform::list_focs))
        .route("/platform/focs/{id}/status", put(platform::set_status))
        .route_layer(from_fn(|req, next| require_roles(SUPER_ADMIN, req, next)))
}
