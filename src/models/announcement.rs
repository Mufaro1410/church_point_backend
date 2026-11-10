use chrono::NaiveDate;
use serde::Deserialize;
use uuid::Uuid;

use super::{common::double_option, scope::Scope};

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low,
    Medium,
    High,
    Urgent,
}

impl Priority {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Urgent => "urgent",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AnnouncementsQuery {
    /// `active=true` keeps only announcements that are switched on and inside their date window.
    #[serde(default)]
    pub active: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateAnnouncementRequest {
    pub title: String,
    pub content: String,
    pub scope: Scope,
    pub priority: Option<Priority>,
    pub category: Option<String>,
    /// Defaults to today.
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAnnouncementRequest {
    pub title: Option<String>,
    pub content: Option<String>,
    pub priority: Option<Priority>,
    #[serde(default, deserialize_with = "double_option")]
    pub category: Option<Option<String>>,
    pub start_date: Option<NaiveDate>,
    #[serde(default, deserialize_with = "double_option")]
    pub end_date: Option<Option<NaiveDate>>,
    pub is_active: Option<bool>,
}

/// A validated announcement ready to insert.
pub struct NewAnnouncement {
    pub title: String,
    pub content: String,
    pub scope: &'static str,
    pub church_id: Uuid,
    pub province_id: Option<Uuid>,
    pub foc_id: Option<Uuid>,
    pub created_by_foc_id: Uuid,
    pub created_by: Uuid,
    pub priority: &'static str,
    pub category: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub is_active: bool,
}
