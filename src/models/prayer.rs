use chrono::NaiveDate;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreatePrayerRequest {
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub is_anonymous: bool,
    /// Defaults to today.
    pub date: Option<NaiveDate>,
}

/// Only the author may edit.
#[derive(Debug, Deserialize)]
pub struct UpdatePrayerRequest {
    pub title: Option<String>,
    pub content: Option<String>,
    pub is_anonymous: Option<bool>,
}
