pub mod app_error;
pub mod domain;

pub use app_error::{AppError, AppResult};
pub use domain::auth::AuthError;
pub use domain::event::EventError;
pub use domain::foc::FocError;
pub use domain::member::MemberError;
