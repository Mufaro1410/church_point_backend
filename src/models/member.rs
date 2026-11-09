use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

/// Maps the `member_profiles` table. We start with the columns we know; add more
/// (phone, dob, ministry_ids, ...) as each screen is migrated.
/// If your ids are not UUIDs, or a column is NOT NULL, adjust the types here.
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

/// Fields an admin may edit. `None` = leave unchanged.
/// Add more fields here as each screen is migrated (phone, dob, ministry_ids, ...).
#[derive(Debug, serde::Deserialize)]
pub struct UpdateMemberRequest {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}
