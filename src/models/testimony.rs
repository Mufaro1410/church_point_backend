use chrono::NaiveDate;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct TestimoniesQuery {
    /// pending | approved | rejected
    pub status: Option<String>,
    /// Max rows (default 200, capped at 500). Newest first.
    pub limit: Option<i64>,
}

// Length limits mirror the database CHECK constraints.
pub const MAX_SITUATION: usize = 150;
pub const MAX_INTERVENTION: usize = 200;
pub const MAX_OUTCOME: usize = 150;

#[derive(Debug, Deserialize)]
pub struct CreateTestimonyRequest {
    pub title: String,
    pub situation: String,
    pub intervention: String,
    pub outcome: String,
    #[serde(default)]
    pub is_anonymous: bool,
    pub date: Option<NaiveDate>,
}

/// Only the author may edit. Any edit sends the testimony back to `pending`.
#[derive(Debug, Deserialize)]
pub struct UpdateTestimonyRequest {
    pub title: Option<String>,
    pub situation: Option<String>,
    pub intervention: Option<String>,
    pub outcome: Option<String>,
    pub is_anonymous: Option<bool>,
}
