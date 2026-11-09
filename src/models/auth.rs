use uuid::Uuid;

/// Row used by the authentication middleware: who is this Supabase user in OUR data?
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AuthProfile {
    pub member_id: Uuid,
    pub foc_id: Option<Uuid>,
    pub church_id: Option<Uuid>,
    pub province_id: Option<Uuid>,
    pub role: Option<String>,
}
