use chrono::NaiveDate;
use serde::Deserialize;
use uuid::Uuid;

use super::common::double_option;

#[derive(Debug, Deserialize)]
pub struct SermonsQuery {
    pub start: Option<NaiveDate>,
    pub end: Option<NaiveDate>,
    /// Lists leave out the (large) `content` text unless `full=true`.
    #[serde(default)]
    pub full: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateSermonRequest {
    pub title: String,
    pub preacher: String,
    pub date: NaiveDate,
    pub content: String,
    pub scripture: Option<String>,
    pub summary: Option<String>,
    pub tags: Option<Vec<String>>,
    pub audio_url: Option<String>,
    pub youtube_link: Option<String>,
    pub facebook_link: Option<String>,
    pub instagram_link: Option<String>,
    pub twitter_link: Option<String>,
    pub website_link: Option<String>,
}

/// Absent = unchanged. `null` (for the optional fields) = clear it.
#[derive(Debug, Deserialize)]
pub struct UpdateSermonRequest {
    pub title: Option<String>,
    pub preacher: Option<String>,
    pub date: Option<NaiveDate>,
    pub content: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub scripture: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub summary: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub tags: Option<Option<Vec<String>>>,
    #[serde(default, deserialize_with = "double_option")]
    pub audio_url: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub youtube_link: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub facebook_link: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub instagram_link: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub twitter_link: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub website_link: Option<Option<String>>,
}

/// A validated sermon ready to insert.
pub struct NewSermon {
    pub foc_id: Uuid,
    pub profile_id: Uuid,
    pub title: String,
    pub preacher: String,
    pub date: NaiveDate,
    pub content: String,
    pub scripture: Option<String>,
    pub summary: Option<String>,
    pub tags: Option<Vec<String>>,
    pub audio_url: Option<String>,
    pub youtube_link: Option<String>,
    pub facebook_link: Option<String>,
    pub instagram_link: Option<String>,
    pub twitter_link: Option<String>,
    pub website_link: Option<String>,
}
