use uuid::Uuid;

/// Row used by the authentication middleware: who is this Supabase user in OUR data?
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AuthProfile {
    pub member_id: Uuid,
    pub foc_id: Option<Uuid>,
    pub church_id: Option<Uuid>,
    pub province_id: Option<Uuid>,
    pub foc_status: Option<String>,
    pub role: Option<String>,
    pub member_status: Option<String>,
    /// Permissions from the member's custom roles ("members.view", ...). Admins need none.
    pub permissions: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForgotRequest {
    pub email: String,
}

/// Sign-up + FOC registration in one call.
#[derive(Debug, serde::Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    #[serde(alias = "churchData")]
    pub church_data: crate::models::catalog::ChurchData,
    #[serde(alias = "userData")]
    pub user_data: crate::models::catalog::UserData,
}
