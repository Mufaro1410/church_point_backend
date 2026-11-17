use axum::{Router, middleware::from_fn, routing::get};

use crate::{handlers::sermon, middleware::authorization::require_roles, state::SharedState};

pub fn routes() -> Router<SharedState> {
    let reads = Router::new()
        .route("/sermons", get(sermon::list))
        .route("/sermons/{id}", get(sermon::get))
        .route_layer(from_fn(|req, next| require_roles(crate::middleware::authorization::ADMIN, req, next)));

    // Creating, editing and deleting sermons: admins only.
    let writes = Router::new()
        .route("/sermons", axum::routing::post(sermon::create))
        .route("/sermons/{id}", axum::routing::patch(sermon::update).delete(sermon::delete))
        .route("/sermons/{id}/audio/upload-url", axum::routing::post(sermon::audio_upload_url))
        .route("/sermons/{id}/audio/confirm", axum::routing::post(sermon::audio_confirm))
        .route("/sermons/{id}/audio", axum::routing::delete(sermon::audio_delete))
        .route_layer(from_fn(|req, next| require_roles(crate::middleware::authorization::ADMIN, req, next)));

    reads.merge(writes)
}
