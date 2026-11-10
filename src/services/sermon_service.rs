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
    utils::validation::{clean_tags, optional_https_url, optional_text, required_text},
};

/// Sermon audio must live in OUR storage bucket, otherwise anyone could save a link to a
/// malicious file that members' apps would then play. Only enforced when SUPABASE_URL is set.
fn audio_prefix(state: &SharedState) -> Option<String> {
    state
        .settings
        .supabase_url
        .as_ref()
        .map(|u| format!("{}/storage/v1/object/public/churchpoint/", u.trim_end_matches('/')))
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
    let prefix = audio_prefix(state);

    let sermon = NewSermon {
        foc_id,
        profile_id: auth.member_id, // who recorded it comes from identity, never from the client
        title: required_text("title", &r.title, 200)?,
        preacher: required_text("preacher", &r.preacher, 150)?,
        date: r.date,
        content: required_text("content", &r.content, 200_000)?,
        scripture: optional_text("scripture", r.scripture, 200)?,
        summary: optional_text("summary", r.summary, 2000)?,
        tags: r.tags.map(clean_tags).transpose()?,
        audio_url: optional_https_url("audio_url", r.audio_url, prefix.as_deref())?,
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
    let prefix = audio_prefix(state);

    r.title = r.title.map(|v| required_text("title", &v, 200)).transpose()?;
    r.preacher = r.preacher.map(|v| required_text("preacher", &v, 150)).transpose()?;
    r.content = r.content.map(|v| required_text("content", &v, 200_000)).transpose()?;
    r.scripture = r.scripture.map(|v| optional_text("scripture", v, 200)).transpose()?;
    r.summary = r.summary.map(|v| optional_text("summary", v, 2000)).transpose()?;
    r.tags = r.tags.map(|v| v.map(clean_tags).transpose()).transpose()?;
    r.audio_url = r.audio_url.map(|v| optional_https_url("audio_url", v, prefix.as_deref())).transpose()?;
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
        _ => Ok(()),
    }
}
