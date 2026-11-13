use axum::{
    Router,
    middleware::from_fn,
    routing::{get, patch, post},
};

use crate::{handlers::event, middleware::authorization::require_roles, state::SharedState};

pub fn routes() -> Router<SharedState> {
    // Reads: admins and cell leaders.
    let reads = Router::new()
        .route("/events", get(event::list))
        .route("/events/{id}", get(event::get))
        .route("/events/{id}/rsvp", get(event::rsvp_stats))
        .route("/events/{id}/attendance", get(event::attendance))
        .route_layer(from_fn(|req, next| require_roles(&["admin"], req, next)));

    // Check-in: admins and cell leaders (they run the door / take the register).
    let check_in = Router::new()
        .route("/events/{id}/attendance", post(event::mark_attendance))
        .route_layer(from_fn(|req, next| require_roles(&["admin"], req, next)));

    // Create / edit / delete: admins only.
    let writes = Router::new()
        .route("/events", post(event::create))
        .route("/events/{id}", patch(event::update).delete(event::delete))
        .route("/events/{id}/qr", post(event::regenerate_qr))
        .route_layer(from_fn(|req, next| require_roles(&["admin"], req, next)));

    reads.merge(check_in).merge(writes)
}
