use axum::{Router, middleware::from_fn, routing::get};

use crate::{handlers::announcement, middleware::authorization::require_roles, state::SharedState};

pub fn routes() -> Router<SharedState> {
    let reads = Router::new()
        .route("/announcements", get(announcement::list))
        .route("/announcements/{id}", get(announcement::get))
        .route_layer(from_fn(|req, next| require_roles(&["admin", "cell_leader"], req, next)));

    let writes = Router::new()
        .route("/announcements", axum::routing::post(announcement::create))
        .route(
            "/announcements/{id}",
            axum::routing::patch(announcement::update).delete(announcement::delete),
        )
        .route_layer(from_fn(|req, next| require_roles(&["admin"], req, next)));

    reads.merge(writes)
}
