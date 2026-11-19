use axum::{Router, middleware::from_fn, routing::{get, patch}};

use crate::{handlers::capacity as h, middleware::authorization::{ADMIN, require_roles}, state::SharedState};

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/serving-capacities", get(h::list).post(h::add))
        .route("/serving-capacities/{id}", patch(h::rename).delete(h::remove))
        .route_layer(from_fn(|req, next| require_roles(ADMIN, req, next)))
}
