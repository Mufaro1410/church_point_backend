use axum::{
    Router,
    middleware::from_fn,
    routing::{get, post},
};

use crate::{handlers::testimony, middleware::authorization::require_perm, state::SharedState};

pub fn routes() -> Router<SharedState> {
    // Authors (admins and cell leaders) read, create, edit and delete; the service enforces ownership.
    let everyone = Router::new()
        .route("/testimonies", get(testimony::list).post(testimony::create))
        .route(
            "/testimonies/{id}",
            get(testimony::get).patch(testimony::update).delete(testimony::delete),
        )
        .route_layer(from_fn(|req, next| require_perm(&["testimonies"], req, next)));

    // Moderation: admins only.
    let moderation = Router::new()
        .route("/testimonies/{id}/approve", post(testimony::approve))
        .route("/testimonies/{id}/reject", post(testimony::reject))
        .route_layer(from_fn(|req, next| require_perm(&["testimonies"], req, next)));

    everyone.merge(moderation)
}
