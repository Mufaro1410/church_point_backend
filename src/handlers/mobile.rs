use axum::{Extension, Json, extract::State, http::StatusCode};
use serde_json::{Map, Value, json};
use uuid::Uuid;

use super::{ApiResponse, extract::{ApiJson, ApiPath, ApiQuery}};
use crate::{
    errors::AppResult,
    middleware::authentication::AuthUser,
    models::mobile::*,
    services::{auth_service, mobile_service as svc},
    state::SharedState,
};

type Reply<T> = Json<ApiResponse<T>>;
type St = State<SharedState>;
type Me = Extension<AuthUser>;

fn ok<T: serde::Serialize>(v: T) -> Reply<T> {
    Json(ApiResponse::ok(v))
}
fn done() -> Reply<Value> {
    ok(json!({ "ok": true }))
}

pub async fn public_focs(State(s): St) -> AppResult<Reply<Vec<Value>>> {
    Ok(ok(svc::public_focs(&s).await?))
}

pub async fn register_member(State(s): St, ApiJson(b): ApiJson<RegisterMemberRequest>) -> AppResult<(StatusCode, Reply<Value>)> {
    Ok((StatusCode::CREATED, ok(auth_service::register_member(&s, b).await?)))
}

pub async fn bootstrap(State(s): St, Extension(a): Me) -> AppResult<Reply<Value>> {
    Ok(ok(svc::bootstrap(&s, &a).await?))
}

pub async fn update_profile(State(s): St, Extension(a): Me, ApiJson(b): ApiJson<Map<String, Value>>) -> AppResult<Reply<Value>> {
    Ok(ok(svc::update_profile(&s, &a, b).await?))
}

pub async fn choose_church(State(s): St, Extension(a): Me, ApiJson(b): ApiJson<ChooseChurch>) -> AppResult<Reply<Value>> {
    svc::choose_church(&s, &a, b).await?;
    Ok(done())
}

pub async fn request_transfer(State(s): St, Extension(a): Me) -> AppResult<Reply<Value>> {
    svc::request_transfer(&s, &a).await?;
    Ok(done())
}

pub async fn cancel_transfer(State(s): St, Extension(a): Me) -> AppResult<Reply<Value>> {
    svc::cancel_transfer(&s, &a).await?;
    Ok(done())
}

pub async fn push_token(State(s): St, Extension(a): Me, ApiJson(b): ApiJson<PushToken>) -> AppResult<Reply<Value>> {
    svc::push_token(&s, &a, b).await?;
    Ok(done())
}

pub async fn rsvp(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>, ApiJson(b): ApiJson<RsvpRequest>) -> AppResult<Reply<Value>> {
    Ok(ok(svc::rsvp(&s, &a, id, b).await?))
}

pub async fn get_notifications(State(s): St, Extension(a): Me) -> AppResult<Reply<Value>> {
    Ok(ok(svc::get_notifications(&s, &a).await?))
}

pub async fn notifications(State(s): St, Extension(a): Me, ApiJson(b): ApiJson<NotificationPrefsRequest>) -> AppResult<Reply<Value>> {
    Ok(ok(svc::set_notifications(&s, &a, b).await?))
}

pub async fn check_in(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>, ApiJson(b): ApiJson<CheckInRequest>) -> AppResult<Reply<Value>> {
    Ok(ok(svc::check_in(&s, &a, id, b.member_id).await?))
}

pub async fn serving(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>, ApiJson(b): ApiJson<ServingRequest>) -> AppResult<Reply<Value>> {
    Ok(ok(svc::serving(&s, &a, id, b).await?))
}

pub async fn react_prayer(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>) -> AppResult<Reply<Value>> {
    Ok(ok(svc::react_prayer(&s, &a, id).await?))
}

pub async fn react_testimony(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>) -> AppResult<Reply<Value>> {
    Ok(ok(svc::react_testimony(&s, &a, id).await?))
}

pub async fn sermon_content(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>) -> AppResult<Reply<Value>> {
    Ok(ok(svc::sermon_content(&s, &a, id).await?))
}

pub async fn get_progress(State(s): St, Extension(a): Me, ApiPath(plan): ApiPath<Uuid>) -> AppResult<Reply<Value>> {
    Ok(ok(svc::get_progress(&s, &a, plan).await?))
}

pub async fn save_progress(State(s): St, Extension(a): Me, ApiPath(plan): ApiPath<Uuid>, ApiJson(b): ApiJson<ProgressRequest>) -> AppResult<Reply<Value>> {
    Ok(ok(svc::save_progress(&s, &a, plan, b).await?))
}

pub async fn my_family(State(s): St, Extension(a): Me) -> AppResult<Reply<Vec<Value>>> {
    Ok(ok(svc::my_family(&s, &a).await?))
}

pub async fn search_members(State(s): St, Extension(a): Me, ApiQuery(q): ApiQuery<SearchQuery>) -> AppResult<Reply<Vec<Value>>> {
    Ok(ok(svc::search_members(&s, &a, q).await?))
}

pub async fn add_family(State(s): St, Extension(a): Me, ApiJson(b): ApiJson<AddFamily>) -> AppResult<(StatusCode, Reply<Value>)> {
    svc::add_family(&s, &a, b).await?;
    Ok((StatusCode::CREATED, done()))
}

pub async fn remove_family(State(s): St, Extension(a): Me, ApiPath(related): ApiPath<Uuid>) -> AppResult<Reply<Value>> {
    svc::remove_family(&s, &a, related).await?;
    Ok(done())
}
