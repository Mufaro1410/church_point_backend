use serde_json::Value;
use uuid::Uuid;

use crate::{
    errors::{AppResult, SermonError},
    middleware::authentication::AuthUser,
    models::{
        common::resolve_range,
        sermon::{CreateSermonRequest, NewSermon, SermonsQuery, UpdateSermonRequest},
    },
    repositories::sermon_repository,
    state::SharedState,
    utils::validation::{clean_tags, optional_block, optional_https_url, optional_line, required_block, required_line},
};

/// Where a sermon's audio lives. Fixed by sermon id, so one FOC can never point at another's file.
fn audio_path(id: Uuid) -> String {
    format!("Sermons/{id}.opus")
}

fn expected_audio_url(state: &SharedState, id: Uuid) -> AppResult<String> {
    state.supabase.public_url(&audio_path(id))
}

pub async fn list(state: &SharedState, auth: &AuthUser, q: SermonsQuery) -> AppResult<Vec<Value>> {
    let foc_id = auth.require_foc()?;
    let (start, end) = resolve_range(q.start, q.end);
    Ok(sermon_repository::list(&state.db, foc_id, start, end, q.full).await?)
}

pub async fn get(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Value> {
    let foc_id = auth.require_foc()?;
    sermon_repository::find(&state.db, id, foc_id)
        .await?
        .ok_or_else(|| SermonError::NotFound.into())
}

pub async fn create(state: &SharedState, auth: &AuthUser, r: CreateSermonRequest) -> AppResult<Value> {
    let foc_id = auth.require_foc()?;
    let sermon = NewSermon {
        foc_id,
        profile_id: auth.member_id, // who recorded it comes from identity, never from the client
        title: required_line("title", &r.title, 200)?,
        preacher: required_line("preacher", &r.preacher, 150)?,
        date: r.date,
        content: required_block("content", &r.content, 200_000)?,
        scripture: optional_line("scripture", r.scripture, 200)?,
        summary: optional_block("summary", r.summary, 2000)?,
        tags: r.tags.map(clean_tags).transpose()?,
        audio_url: if r.audio_url.as_deref().is_some_and(|a| !a.trim().is_empty()) {
            return Err(crate::errors::AppError::Validation("Create the sermon first, then upload its audio.".into()));
        } else {
            None
        },
        youtube_link: optional_https_url("youtube_link", r.youtube_link, None)?,
        facebook_link: optional_https_url("facebook_link", r.facebook_link, None)?,
        instagram_link: optional_https_url("instagram_link", r.instagram_link, None)?,
        twitter_link: optional_https_url("twitter_link", r.twitter_link, None)?,
        website_link: optional_https_url("website_link", r.website_link, None)?,
    };
    Ok(sermon_repository::insert(&state.db, &sermon).await?)
}

pub async fn update(
    state: &SharedState,
    auth: &AuthUser,
    id: Uuid,
    mut r: UpdateSermonRequest,
) -> AppResult<Value> {
    let foc_id = auth.require_foc()?;
    r.title = r.title.map(|v| required_line("title", &v, 200)).transpose()?;
    r.preacher = r.preacher.map(|v| required_line("preacher", &v, 150)).transpose()?;
    r.content = r.content.map(|v| required_block("content", &v, 200_000)).transpose()?;
    r.scripture = r.scripture.map(|v| optional_line("scripture", v, 200)).transpose()?;
    r.summary = r.summary.map(|v| optional_block("summary", v, 2000)).transpose()?;
    r.tags = r.tags.map(|v| v.map(clean_tags).transpose()).transpose()?;
    if let Some(Some(url)) = r.audio_url.as_ref().map(|v| v.as_deref().map(str::trim).filter(|u| !u.is_empty())) {
        if url != expected_audio_url(state, id)? {
            return Err(crate::errors::AppError::Validation("audio_url can only be set by uploading audio for this sermon.".into()));
        }
    }
    r.youtube_link = r.youtube_link.map(|v| optional_https_url("youtube_link", v, None)).transpose()?;
    r.facebook_link = r.facebook_link.map(|v| optional_https_url("facebook_link", v, None)).transpose()?;
    r.instagram_link = r.instagram_link.map(|v| optional_https_url("instagram_link", v, None)).transpose()?;
    r.twitter_link = r.twitter_link.map(|v| optional_https_url("twitter_link", v, None)).transpose()?;
    r.website_link = r.website_link.map(|v| optional_https_url("website_link", v, None)).transpose()?;

    sermon_repository::update(&state.db, id, foc_id, &r)
        .await?
        .ok_or_else(|| SermonError::NotFound.into())
}

pub async fn delete(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<()> {
    let foc_id = auth.require_foc()?;
    match sermon_repository::delete(&state.db, id, foc_id).await? {
        0 => Err(SermonError::NotFound.into()),
        _ => {
            // Best effort: a leftover file is harmless, a failed delete must not undo the sermon delete.
            if let Err(e) = state.supabase.remove_object(&audio_path(id)).await {
                tracing::warn!(error = ?e, "could not remove sermon audio");
            }
            Ok(())
        }
    }
}

/// Step 1 of an audio upload: Axum checks the sermon is yours, then hands the browser a one-time upload URL.
pub async fn audio_upload_url(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Value> {
    get(state, auth, id).await?;
    let path = audio_path(id);
    let upload_url = state.supabase.sign_upload(&path).await?;
    Ok(serde_json::json!({
        "upload_url": upload_url,
        "method": "PUT",
        "content_type": "audio/ogg; codecs=opus",
        "public_url": expected_audio_url(state, id)?,
    }))
}

/// Step 2: after the browser has uploaded, Axum checks the file really exists, then links it.
pub async fn audio_confirm(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Value> {
    let foc_id = auth.require_foc()?;
    get(state, auth, id).await?;
    if !state.supabase.object_exists(&audio_path(id)).await? {
        return Err(crate::errors::AppError::Validation("No uploaded audio was found for this sermon.".into()));
    }
    let url = expected_audio_url(state, id)?;
    sermon_repository::set_audio(&state.db, id, foc_id, Some(&url)).await?.ok_or_else(|| SermonError::NotFound.into())
}

pub async fn audio_delete(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<Value> {
    let foc_id = auth.require_foc()?;
    get(state, auth, id).await?;
    state.supabase.remove_object(&audio_path(id)).await?;
    sermon_repository::set_audio(&state.db, id, foc_id, None).await?.ok_or_else(|| SermonError::NotFound.into())
}
