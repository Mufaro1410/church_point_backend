use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde::Deserialize;
use uuid::Uuid;

use crate::errors::AuthError;

/// The parts of a Supabase access token we care about.
#[derive(Debug, Deserialize)]
pub struct SupabaseClaims {
    /// auth.users.id
    pub sub: Uuid,
    pub exp: i64,
    pub email: Option<String>,
}

/// Verifies a token issued by Supabase Auth (the one supabase-js gives your React app).
/// NOTE: this is for projects using the legacy shared HS256 JWT secret. If your project
/// uses asymmetric JWT signing keys, we'll switch this to JWKS verification.
pub fn verify_supabase_token(token: &str, secret: &str) -> Result<SupabaseClaims, AuthError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_audience(&["authenticated"]);
    decode::<SupabaseClaims>(token, &DecodingKey::from_secret(secret.as_bytes()), &validation)
        .map(|d| d.claims)
        .map_err(|_| AuthError::InvalidToken)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{EncodingKey, Header, encode};
    use serde_json::json;

    const SECRET: &str = "test-secret";

    fn make(aud: &str, exp: i64, secret: &str) -> String {
        let claims = json!({ "sub": Uuid::new_v4(), "exp": exp, "aud": aud, "email": "a@b.com" });
        encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes())).unwrap()
    }
    fn future() -> i64 { chrono::Utc::now().timestamp() + 3600 }

    #[test]
    fn accepts_valid_token() {
        assert!(verify_supabase_token(&make("authenticated", future(), SECRET), SECRET).is_ok());
    }
    #[test]
    fn rejects_wrong_secret() {
        assert!(verify_supabase_token(&make("authenticated", future(), "other"), SECRET).is_err());
    }
    #[test]
    fn rejects_expired() {
        let past = chrono::Utc::now().timestamp() - 3600;
        assert!(verify_supabase_token(&make("authenticated", past, SECRET), SECRET).is_err());
    }
    #[test]
    fn rejects_wrong_audience() {
        assert!(verify_supabase_token(&make("anon", future(), SECRET), SECRET).is_err());
    }
}
