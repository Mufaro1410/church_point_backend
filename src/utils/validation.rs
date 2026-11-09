use crate::errors::AppError;

/// Trims and requires a non-empty value.
pub fn non_empty(field: &str, value: &str) -> Result<String, AppError> {
    let v = value.trim();
    if v.is_empty() {
        return Err(AppError::Validation(format!("{field} is required.")));
    }
    Ok(v.to_string())
}
