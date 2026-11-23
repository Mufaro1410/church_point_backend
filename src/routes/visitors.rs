use axum::{Router, middleware::from_fn, routing::{get, post}};

use crate::{handlers::visitor as h, middleware::authorization::require_perm, state::SharedState};

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/visitors", get(h::list).post(h::add))
        .route("/visitors/form-link", get(h::form_link).put(h::set_form_link))
        .route("/visitors/{id}", get(h::get).patch(h::update).delete(h::remove))
        .route("/visitors/{id}/follow-ups", get(h::visitor_log).post(h::visitor_log_add))
        .route("/visitors/{id}/convert", post(h::convert))
        .route("/follow-up/absent", get(h::absent))
        .route("/follow-up/members/{id}", get(h::member_log).post(h::member_log_add))
        .route_layer(from_fn(|req, next| require_perm(&["visitors"], req, next)))
}

/// No sign-in needed: guests scan the QR code and fill this in on their own phones.
pub fn public_routes() -> Router<SharedState> {
    Router::new().route("/public/visit/{token}", get(h::public_church).post(h::public_submit))
}
