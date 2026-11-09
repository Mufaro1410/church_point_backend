use axum::{Router, routing::get};

use crate::{handlers::auth, state::SharedState};

pub fn routes() -> Router<SharedState> {
    Router::new().route("/auth/me", get(auth::me))
}
