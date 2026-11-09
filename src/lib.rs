pub mod config;
pub mod errors;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod repositories;
pub mod routes;
pub mod services;
pub mod state;
pub mod utils;

use axum::Router;

use crate::state::SharedState;

/// Builds the full app. Used by main.rs AND by integration tests.
pub fn build_app(state: SharedState) -> Router {
    routes::build(state)
}
