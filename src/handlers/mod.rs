use serde::Serialize;

pub mod auth;
pub mod event;
pub mod extract;
pub mod foc;
pub mod health;
pub mod member;

/// Standard success envelope: { "success": true, "data": ... }
#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: T,
}
impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self { Self { success: true, data } }
}
