use axum::{Router, middleware::from_fn, routing::{delete, get}};

use crate::{
    handlers::org,
    middleware::authorization::require_perm,
    state::SharedState,
};

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/cell-groups", get(org::list_groups).post(org::create_group))
        .route("/cell-groups/{id}", get(org::get_group).patch(org::update_group).delete(org::delete_group))
        .route("/cell-groups/{id}/meetings", get(org::list_meetings).post(org::create_meeting))
        .route("/cell-meetings", get(org::list_all_meetings))
        .route("/cell-meetings/{id}", delete(org::delete_meeting).patch(org::update_meeting))
        .route_layer(from_fn(|req, next| require_perm(&["cell_groups"], req, next)))
        .merge(
            Router::new()
                .route("/dashboard", get(org::dashboard))
                .route_layer(from_fn(|req, next| require_perm(&["reports"], req, next))),
        )
}
