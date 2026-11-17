use axum::{
    Router,
    middleware::from_fn,
    routing::{delete, get, post},
};

use crate::{
    handlers::member as h,
    middleware::authorization::{ADMIN, require_roles},
    state::SharedState,
};

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/members", get(h::list).post(h::create))
        .route("/members/{id}", get(h::get).patch(h::update).delete(h::delete))
        .route("/members/{id}/approve", post(h::approve))
        .route("/members/{id}/pending", post(h::set_pending))
        .route("/members/{id}/reject", post(h::reject))
        .route("/members/{id}/release", post(h::release))
        .route("/members/{id}/role", axum::routing::put(h::set_role))
        .route("/members/{id}/family", get(h::family).post(h::add_family))
        .route("/members/{id}/contributions", get(h::contributions))
        .route("/members/{id}/attendance", get(h::attendance))
        .route("/members/{id}/activity", get(h::activity))
        .route("/family/{id}", delete(h::remove_family))
        .route_layer(from_fn(|req, next| require_roles(ADMIN, req, next)))
}
