use serde_json::Value;
use uuid::Uuid;

use crate::{
    errors::{AppError, AppResult, AuthError, TestimonyError},
    middleware::authentication::AuthUser,
    models::testimony::{
        CreateTestimonyRequest, MAX_INTERVENTION, MAX_OUTCOME, MAX_SITUATION, TestimoniesQuery,
        UpdateTestimonyRequest,
    },
    repositories::{member_repository, testimony_repository},
    state::SharedState,
    utils::validation::required_text,
};

pub async fn list(state: &SharedState, auth: &AuthUser, q: TestimoniesQuery) -> AppResult<Vec<Value>> {
    let foc_id = auth.require_foc()?;
    let status = match q.status.as_deref() {
        None => None,
        Some(s @ ("pending" | "approved" | "rejected")) => Some(s),
        Some(_) => return Err(AppError::Validation("status must be pending, approved or rejected.".into())),
    };
    let limit = q.limit.unwrap_or(200).clamp(1, 500);
    Ok(testimony_repository::list(&state.db, foc_id, auth.member_id, status, limit).await?)
}

pub async fn get(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Value> {
    let foc_id = auth.require_foc()?;
    testimony_repository::find(&state.db, id, foc_id, auth.member_id)
        .await?
        .ok_or_else(|| TestimonyError::NotFound.into())
}

pub async fn create(state: &SharedState, auth: &AuthUser, r: CreateTestimonyRequest) -> AppResult<Value> {
    let foc_id = auth.require_foc()?;
    let title = required_text("title", &r.title, 150)?;
    let situation = required_text("situation", &r.situation, MAX_SITUATION)?;
    let intervention = required_text("intervention", &r.intervention, MAX_INTERVENTION)?;
    let outcome = required_text("outcome", &r.outcome, MAX_OUTCOME)?;

    // Author name and id come from the caller's identity, never from the request.
    let author = member_repository::full_name(&state.db, auth.member_id)
        .await?
        .unwrap_or_else(|| "Member".to_string());

    let id = testimony_repository::insert(
        &state.db, foc_id, auth.member_id, &author, &title, &situation, &intervention, &outcome,
        r.is_anonymous, r.date,
    )
    .await?;
    get(state, auth, id).await
}

async fn why_failed(state: &SharedState, foc_id: Uuid, id: Uuid) -> AppResult<AppError> {
    Ok(if testimony_repository::exists(&state.db, id, foc_id).await? {
        AuthError::Forbidden.into()
    } else {
        TestimonyError::NotFound.into()
    })
}

/// Author only. Any edit sends the testimony back to `pending` for re-approval.
pub async fn update(state: &SharedState, auth: &AuthUser, id: Uuid, mut r: UpdateTestimonyRequest) -> AppResult<Value> {
    let foc_id = auth.require_foc()?;
    r.title = r.title.map(|v| required_text("title", &v, 150)).transpose()?;
    r.situation = r.situation.map(|v| required_text("situation", &v, MAX_SITUATION)).transpose()?;
    r.intervention = r.intervention.map(|v| required_text("intervention", &v, MAX_INTERVENTION)).transpose()?;
    r.outcome = r.outcome.map(|v| required_text("outcome", &v, MAX_OUTCOME)).transpose()?;

    match testimony_repository::update_own(&state.db, id, foc_id, auth.member_id, &r).await? {
        Some(id) => get(state, auth, id).await,
        None => Err(why_failed(state, foc_id, id).await?),
    }
}

async fn moderate(state: &SharedState, auth: &AuthUser, id: Uuid, status: &'static str) -> AppResult<Value> {
    let foc_id = auth.require_foc()?;
    match testimony_repository::moderate(&state.db, id, foc_id, auth.member_id, status).await? {
        Some(id) => get(state, auth, id).await,
        None => Err(TestimonyError::NotFound.into()),
    }
}

pub async fn approve(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Value> {
    moderate(state, auth, id, "approved").await
}

pub async fn reject(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Value> {
    moderate(state, auth, id, "rejected").await
}

/// The author, or an admin moderating.
pub async fn delete(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<()> {
    let foc_id = auth.require_foc()?;
    match testimony_repository::delete(&state.db, id, foc_id, auth.member_id, auth.can("testimonies.delete")).await? {
        0 => Err(why_failed(state, foc_id, id).await?),
        _ => Ok(()),
    }
}
