use axum::{Router, routing::get};

use crate::{handlers::foc, state::SharedState};

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/foc", get(foc::current))
        .route("/provinces", get(foc::provinces))
}
