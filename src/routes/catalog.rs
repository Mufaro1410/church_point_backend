use axum::{
    Router,
    middleware::from_fn,
    routing::{delete, get, put},
};

use crate::{
    handlers::catalog as h,
    middleware::authorization::{SUPER_ADMIN, require_perm, require_roles},
    state::SharedState,
};

pub fn routes() -> Router<SharedState> {
    use axum::routing::patch;
    let guard = |areas: &'static [&'static str]| from_fn(move |req, next| require_perm(areas, req, next));
    let ministries = Router::new()
        .route("/ministries", get(h::ministries).post(h::add_ministry))
        .route("/ministries/{id}/selected", put(h::select_ministry))
        .route("/ministries/{id}/leader", put(h::set_leader))
        .route_layer(guard(&["ministries", "members"]));
    let courses = Router::new()
        .route("/courses", get(h::courses).post(h::add_course))
        .route_layer(guard(&["courses", "members"]));
    let events = Router::new()
        .route("/events/{id}/serving", get(h::list_serving).post(h::add_serving))
        .route("/event-serving", get(h::all_serving))
        .route("/event-serving/{id}", delete(h::remove_serving))
        .route("/events/{id}/services", get(h::list_services).post(h::add_service))
        .route("/event-services/{id}", delete(h::remove_service))
        .route_layer(guard(&["events"]));
    let social = Router::new()
        .route("/social-events", get(h::social_list).post(h::social_create))
        .route("/social-events/{id}", patch(h::social_update).delete(h::social_delete))
        .route_layer(guard(&["social_events"]));
    let plans = Router::new()
        .route("/bible-plans", get(h::plans).post(h::plan_create))
        .route("/bible-plans/{id}", patch(h::plan_update).delete(h::plan_delete))
        .route_layer(guard(&["dbr"]));
    // Lookups every staff member's screens need.
    let lookups = Router::new()
        .route("/logos", get(h::logos))
        .route("/church", get(h::church))
        .route("/focs", get(h::focs))
        .route_layer(guard(&["*"]));
    let foc = Router::new().route("/foc", patch(h::update_foc)).route_layer(guard(&["settings"]));
    // Shared catalogues: only the platform owner renames or removes entries.
    let owner = Router::new()
        .route("/ministries/{id}", patch(h::rename_ministry).delete(h::delete_ministry))
        .route("/courses/{id}", patch(h::rename_course).delete(h::delete_course))
        .route_layer(from_fn(|req, next| require_roles(SUPER_ADMIN, req, next)));
    ministries.merge(courses).merge(events).merge(social).merge(plans).merge(lookups).merge(foc).merge(owner)
}
