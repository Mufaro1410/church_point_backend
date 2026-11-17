use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Small projection used internally (e.g. by events). Full profiles are returned as JSON.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Member {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub foc_id: Option<Uuid>,
    pub email: Option<String>,
    pub role: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MemberQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub cell_group_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseRequest {
    pub status: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleRequest {
    pub role: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyRequest {
    pub related_member_id: Uuid,
    pub relationship_type: String,
}
