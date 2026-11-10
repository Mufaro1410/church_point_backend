use axum::{Router, middleware::from_fn_with_state, routing::get};
use tower_http::{
    compression::CompressionLayer, cors::CorsLayer, limit::RequestBodyLimitLayer,
    trace::TraceLayer,
};

use crate::{
    handlers::health,
    middleware::{authentication::authenticate, logging, request_id},
    state::SharedState,
};

pub mod announcements;
pub mod auth;
pub mod events;
pub mod foc;
pub mod members;
pub mod prayers;
pub mod sermons;
pub mod testimonies;

fn cors_layer(origins: &[String]) -> CorsLayer {
    use axum::http::{HeaderValue, Method, header};
    let parsed: Vec<HeaderValue> = origins.iter().filter_map(|o| o.parse().ok()).collect();
    CorsLayer::new()
        .allow_origin(parsed)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
}

pub fn build(state: SharedState) -> Router {
    // Every route registered here requires a valid Supabase token.
    let api = Router::new()
        .merge(auth::routes())
        .merge(events::routes())
        .merge(foc::routes())
        .merge(members::routes())
        .merge(sermons::routes())
        .merge(prayers::routes())
        .merge(testimonies::routes())
        .merge(announcements::routes())
        .route_layer(from_fn_with_state(state.clone(), authenticate));

    Router::new()
        .route("/health", get(health::health))
        .route("/ready", get(health::ready))
        .nest("/api/v1", api)
        .with_state(state.clone())
        // Layers added last run first (outermost).
        .layer(cors_layer(&state.settings.cors_origins))
        .layer(CompressionLayer::new())
        .layer(RequestBodyLimitLayer::new(1024 * 1024))
        .layer(request_id::propagate_layer())
        .layer(TraceLayer::new_for_http().make_span_with(logging::make_span))
        .layer(request_id::set_layer())
}
