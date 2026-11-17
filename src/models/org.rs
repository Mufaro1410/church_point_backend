use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellGroupRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub meeting_day: Option<String>,
    /// "HH:MM"
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub leader_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeetingRequest {
    /// "YYYY-MM-DD"
    pub date: Option<String>,
    pub title: Option<String>,
    pub notes: Option<String>,
    /// JSON arrays, stored as-is.
    pub attendance: Option<serde_json::Value>,
    pub offering: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct StatusRequest {
    pub status: String,
}
