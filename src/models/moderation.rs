use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportRequest {
    pub kind: String,
    pub content_id: Uuid,
    pub reason: Option<String>,
}

/// Block the author of a post (works for anonymous posts without revealing who), or a named member.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockRequest {
    pub kind: Option<String>,
    pub content_id: Option<Uuid>,
    pub member_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptTermsRequest {
    /// Required for members under 13: the parent or guardian who agrees for them.
    pub guardian_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionRequest {
    /// dismiss | remove | remove_and_eject
    pub action: String,
}
