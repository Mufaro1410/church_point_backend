use axum::{
    Router,
    middleware::from_fn,
    routing::{get, post},
};

use crate::{handlers::member, middleware::authorization::require_roles, state::SharedState};

pub fn routes() -> Router<SharedState> {
    // Reads: admins and cell leaders.
    let reads = Router::new()
        .route("/members", get(member::list))
        .route("/members/{id}", get(member::get))
        .route_layer(from_fn(|req, next| require_roles(&["admin", "cell_leader"], req, next)));

    // Writes: admins only. Permissions sit right next to the routes they protect.
    let writes = Router::new()
        .route("/members/{id}", axum::routing::patch(member::update).delete(member::delete))
        .route("/members/{id}/approve", post(member::approve))
        .route("/members/{id}/reject", post(member::reject))
        .route_layer(from_fn(|req, next| require_roles(&["admin"], req, next)));

    reads.merge(writes)
}
