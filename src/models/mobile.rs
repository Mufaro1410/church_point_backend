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

/// Push notification switches. Anything left out stays as it was.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationPrefsRequest {
    pub enabled: Option<bool>,
    pub announcements: Option<bool>,
    pub events: Option<bool>,
}

/// Check in yourself (empty body) or a family member you are linked to.
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CheckInRequest {
    pub member_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServingRequest {
    pub ministry_id: Option<Uuid>,
    pub capacity: Option<String>,
    /// Several roles at once (replaces what the member had for this event).
    pub servings: Option<Vec<ServingItem>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServingItem {
    pub ministry_id: Option<Uuid>,
    pub capacity: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteAccountRequest {
    #[serde(default)]
    pub reason: String,
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
