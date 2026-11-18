use axum::{Router, routing::{get, post}};

use crate::{handlers::auth, state::SharedState};

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/auth/me", get(auth::me))
        .route("/auth/profile", get(auth::profile))
        .route("/auth/password", axum::routing::put(auth::change_password))
}

pub fn token_routes() -> Router<SharedState> {
    Router::new()
        .route("/auth/register-foc", post(auth::register_foc))
        .route("/auth/logout", post(auth::logout))
}

/// No token needed: these are how a person gets one.
pub fn public_routes() -> Router<SharedState> {
    Router::new()
        .route("/auth/login", post(auth::login))
        .route("/auth/refresh", post(auth::refresh))
        .route("/auth/register", post(auth::register))
        .route("/auth/forgot-password", post(auth::forgot_password))
}
