use axum::{Router, middleware::from_fn, routing::{get, post}};

use crate::{
    handlers::payment,
    middleware::authorization::require_perm,
    state::SharedState,
};

pub fn routes() -> Router<SharedState> {
    // Church admins: set up Paynow and see what came in.
    let admin = Router::new()
        .route("/payments/settings", get(payment::get_settings).put(payment::put_settings))
        .route("/payments", get(payment::list))
        .route_layer(from_fn(|req, next| require_perm(&["payments"], req, next)));

    // Any signed-in member of the church can pay and check their own payment.
    let members = Router::new()
        .route("/payments", post(payment::start))
        .route("/payments/{id}", get(payment::get));

    admin.merge(members)
}

/// Paynow calling us back (no login — verified by signature).
pub fn public_routes() -> Router<SharedState> {
    Router::new().route("/webhooks/paynow/{id}", post(payment::webhook))
}
