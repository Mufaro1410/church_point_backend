use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterMemberRequest {
    pub email: String,
    pub password: String,
    pub first_name: String,
    pub last_name: String,
    pub gender: String,
    pub phone: String,
    pub foc_id: Uuid,
    /// The person must tick the box; the server records the time.
    pub accepted_terms: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChooseChurch {
    pub foc_id: Uuid,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PushToken {
    pub token: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RsvpRequest {
    /// going | maybe | notGoing
    pub status: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServingRequest {
    pub ministry_id: Option<Uuid>,
    pub capacity: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgressRequest {
    /// Merged into the saved progress, e.g. { "12": { "isRead": true, "completed_at": "…" } }
    pub progress: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddFamily {
    pub related_member_id: Uuid,
    pub relationship_type: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}
