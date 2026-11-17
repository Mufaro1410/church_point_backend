use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedRequest {
    pub selected: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeaderRequest {
    pub leader_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServingRequest {
    pub profile_id: Uuid,
    pub ministry_id: Option<Uuid>,
    pub capacity: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChurchData {
    pub name: String,
    pub address: String,
    pub email: String,
    pub contact: String,
    pub church_id: Uuid,
    pub country: Option<String>,
    pub logo: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UserData {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone: String,
    pub gender: String,
    pub address: Option<String>,
    pub dob: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterFoc {
    #[serde(alias = "churchData")]
    pub church_data: ChurchData,
    #[serde(alias = "userData")]
    pub user_data: UserData,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceRequest {
    pub member_id: Uuid,
    pub service_role: String,
    pub service_domain: Option<String>,
    pub notes: Option<String>,
}
