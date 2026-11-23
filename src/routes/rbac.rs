use axum::{Router, middleware::from_fn, routing::{get, patch}};

use crate::{handlers::rbac as h, middleware::authorization::{ADMIN, require_roles}, state::SharedState};

/// Roles and age groups are set up by church admins only (roles cannot hand out the power to make roles).
pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/permissions", get(h::permissions))
        .route("/roles", get(h::roles).post(h::add_role))
        .route("/roles/{id}", patch(h::update_role).delete(h::delete_role))
        .route("/roles/{id}/members", get(h::role_members))
        .route("/members/{id}/roles", get(h::member_roles).put(h::set_member_roles))
        .route("/age-groups", get(h::age_groups).post(h::add_age_group))
        .route("/age-groups/{id}", patch(h::update_age_group).delete(h::delete_age_group))
        .route_layer(from_fn(|req, next| require_roles(ADMIN, req, next)))
}
