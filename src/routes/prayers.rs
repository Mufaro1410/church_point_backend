use axum::{Router, middleware::from_fn, routing::get};

use crate::{handlers::prayer, middleware::authorization::require_roles, state::SharedState};

/// Admins and cell leaders may use these routes; the service then enforces that only the
/// author edits, and only the author or an admin deletes.
pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/prayers", get(prayer::list).post(prayer::create))
        .route("/prayers/{id}", get(prayer::get).patch(prayer::update).delete(prayer::delete))
        .route_layer(from_fn(|req, next| require_roles(&["admin", "cell_leader"], req, next)))
}
