//! Member (mobile app) routes. Everything lives under /m and needs a signed-in, accepted member.

use axum::{
    Router,
    extract::Request,
    middleware::{Next, from_fn},
    response::Response,
    routing::{delete, get, patch, post, put},
};

use crate::{
    errors::{AppError, AuthError},
    handlers::{mobile as h, prayer, testimony},
    middleware::authentication::AuthUser,
    services::mobile_service::require_active,
    state::SharedState,
};

async fn active_member(req: Request, next: Next) -> Result<Response, AppError> {
    let user = req.extensions().get::<AuthUser>().ok_or(AuthError::MissingToken)?;
    require_active(user)?;
    Ok(next.run(req).await)
}

pub fn public_routes() -> Router<SharedState> {
    Router::new()
        .route("/public/focs", get(h::public_focs))
        .route("/auth/register-member", post(h::register_member))
}

pub fn routes() -> Router<SharedState> {
    // A signed-in person who has no church yet (or is waiting) can pick one.
    let signed_in = Router::new()
        .route("/m/church", put(h::choose_church))
        .route("/m/me", get(h::my_profile))
        .route("/m/account/delete", post(h::delete_account));

    let members = Router::new()
        .route("/m/bootstrap", get(h::bootstrap))
        .route("/m/profile", patch(h::update_profile))
        .route("/m/transfer/request", post(h::request_transfer))
        .route("/m/transfer/cancel", post(h::cancel_transfer))
        .route("/m/push-token", put(h::push_token))
        .route("/m/notifications", get(h::get_notifications).put(h::notifications))
        .route("/m/events/{id}/rsvp", put(h::rsvp))
        .route("/m/events/{id}/check-in", post(h::check_in))
        .route("/m/events/{id}/serving", put(h::serving))
        .route("/m/prayers", post(prayer::create))
        .route("/m/prayers/{id}", patch(prayer::update).delete(prayer::delete))
        .route("/m/prayers/{id}/support", post(h::react_prayer))
        .route("/m/testimonies", post(testimony::create))
        .route("/m/testimonies/{id}", patch(testimony::update).delete(testimony::delete))
        .route("/m/testimonies/{id}/react", post(h::react_testimony))
        .route("/m/sermons/{id}/content", get(h::sermon_content))
        .route("/m/bible-progress/{plan_id}", get(h::get_progress).put(h::save_progress))
        .route("/m/family", get(h::my_family).post(h::add_family))
        .route("/m/family/{related_id}", delete(h::remove_family))
        .route("/m/members/search", get(h::search_members))
        .route_layer(from_fn(active_member));

    signed_in.merge(members)
}
