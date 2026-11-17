use axum::{
    Router,
    middleware::from_fn,
    routing::{delete, get, put},
};

use crate::{
    handlers::catalog as h,
    middleware::authorization::{ADMIN, SUPER_ADMIN, require_roles},
    state::SharedState,
};

pub fn routes() -> Router<SharedState> {
    let admin = Router::new()
        .route("/ministries", get(h::ministries).post(h::add_ministry))
        .route("/ministries/{id}/selected", put(h::select_ministry))
        .route("/ministries/{id}/leader", put(h::set_leader))
        .route("/courses", get(h::courses).post(h::add_course))
        .route("/events/{id}/serving", get(h::list_serving).post(h::add_serving))
        .route("/event-serving", get(h::all_serving))
        .route("/event-serving/{id}", delete(h::remove_serving))
        .route("/events/{id}/services", get(h::list_services).post(h::add_service))
        .route("/event-services/{id}", delete(h::remove_service))
        .route("/logos", get(h::logos))
        .route("/social-events", get(h::social_list).post(h::social_create))
        .route("/social-events/{id}", axum::routing::patch(h::social_update).delete(h::social_delete))
        .route("/bible-plans", get(h::plans).post(h::plan_create))
        .route("/bible-plans/{id}", axum::routing::patch(h::plan_update).delete(h::plan_delete))
        .route("/church", get(h::church))
        .route("/focs", get(h::focs))
        .route("/foc", axum::routing::patch(h::update_foc))
        .route_layer(from_fn(|req, next| require_roles(ADMIN, req, next)));

    // Shared catalogues: only the platform owner renames or removes entries.
    let owner = Router::new()
        .route("/ministries/{id}", axum::routing::patch(h::rename_ministry).delete(h::delete_ministry))
        .route("/courses/{id}", axum::routing::patch(h::rename_course).delete(h::delete_course))
        .route_layer(from_fn(|req, next| require_roles(SUPER_ADMIN, req, next)));

    admin.merge(owner)
}
