use serde::Serialize;

pub mod announcement;
pub mod auth;
pub mod catalog;
pub mod event;
pub mod extract;
pub mod finance;
pub mod foc;
pub mod health;
pub mod member;
pub mod org;
pub mod platform;
pub mod prayer;
pub mod sermon;
pub mod testimony;

/// Standard success envelope: { "success": true, "data": ... }
#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: T,
}
impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self { Self { success: true, data } }
}
pub mod payment;
pub mod mobile;
pub mod capacity;
