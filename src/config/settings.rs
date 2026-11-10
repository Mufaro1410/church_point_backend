use std::env;

#[derive(Debug, Clone)]
pub struct Settings {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub db_max_connections: u32,
    pub supabase_jwt_secret: String,
    /// Optional. When set, sermon `audio_url`s must point into this project's storage.
    pub supabase_url: Option<String>,
    pub cors_origins: Vec<String>,
}

impl Settings {
    /// Fails fast at startup so misconfiguration never reaches runtime.
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            host: env::var("HOST").unwrap_or_else(|_| "127.0.0.1".into()),
            port: parse_or("PORT", 8080)?,
            database_url: required("DATABASE_URL")?,
            db_max_connections: parse_or("DB_MAX_CONNECTIONS", 10)?,
            supabase_jwt_secret: required("SUPABASE_JWT_SECRET")?,
            supabase_url: env::var("SUPABASE_URL").ok().filter(|s| !s.trim().is_empty()),
            cors_origins: env::var("CORS_ORIGINS")
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        })
    }
}

fn required(key: &str) -> Result<String, String> {
    env::var(key).map_err(|_| format!("Missing required environment variable: {key}"))
}

fn parse_or<T: std::str::FromStr>(key: &str, default: T) -> Result<T, String> {
    match env::var(key) {
        Ok(v) => v.parse().map_err(|_| format!("Invalid value for {key}: {v}")),
        Err(_) => Ok(default),
    }
}
