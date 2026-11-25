use std::env;

#[derive(Debug, Clone)]
pub struct Settings {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub db_max_connections: u32,
    /// Connections kept open and ready, so the first requests do not wait for a new TLS connection.
    pub db_min_connections: u32,
    /// Seconds to remember who a token belongs to (0 = always look it up).
    pub auth_cache_secs: u64,
    /// Path or name of the ffmpeg program used to shrink sermon audio.
    pub ffmpeg_bin: String,
    /// Biggest audio file accepted for upload, in megabytes.
    pub audio_max_mb: usize,
    pub supabase_jwt_secret: String,
    /// Optional. When set, sermon `audio_url`s must point into this project's storage.
    pub supabase_url: Option<String>,
    /// Public (anon) key: lets Axum call Supabase Auth on behalf of users (login, refresh, sign-up).
    pub supabase_anon_key: Option<String>,
    /// Secret key: only Axum holds it (storage signing, deleting files, cleaning up failed sign-ups).
    pub supabase_service_key: Option<String>,
    pub cors_origins: Vec<String>,
    /// 32-byte key (base64) that protects each church's Paynow key in the database.
    pub payments_key: Option<[u8; 32]>,
    /// Public https address of THIS server, so Paynow can report results (e.g. a tunnel while testing).
    pub public_base_url: Option<String>,
    /// Paynow address (override only for testing).
    pub paynow_base_url: String,
    /// Expo push service (sends to the members' phones).
    pub expo_push_url: String,
    /// Event reminders on/off, and the church's local time zone (reminders go out at local 17:00 the day before and 07:00 on the day).
    pub reminders_enabled: bool,
    pub app_timezone: String,
    /// Page the "reset password" email links to (optional; otherwise Supabase's own setting is used).
    pub password_reset_redirect: Option<String>,
    /// Where the "confirm your email" link lands after sign-up (optional).
    pub email_confirm_redirect: Option<String>,
}

impl Settings {
    /// Fails fast at startup so misconfiguration never reaches runtime.
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            host: env::var("HOST").unwrap_or_else(|_| "127.0.0.1".into()),
            port: parse_or("PORT", 8080)?,
            database_url: required("DATABASE_URL")?,
            db_max_connections: parse_or("DB_MAX_CONNECTIONS", 10)?,
            db_min_connections: parse_or("DB_MIN_CONNECTIONS", 4)?,
            auth_cache_secs: parse_or("AUTH_CACHE_SECONDS", 10)?,
            ffmpeg_bin: env::var("FFMPEG_BIN").unwrap_or_else(|_| "ffmpeg".into()),
            audio_max_mb: parse_or("AUDIO_MAX_MB", 200)?,
            supabase_jwt_secret: required("SUPABASE_JWT_SECRET")?,
            supabase_url: env::var("SUPABASE_URL").ok().filter(|s| !s.trim().is_empty()),
            supabase_anon_key: env::var("SUPABASE_ANON_KEY").ok().filter(|s| !s.trim().is_empty()),
            supabase_service_key: env::var("SUPABASE_SERVICE_ROLE_KEY").ok().filter(|s| !s.trim().is_empty()),
            payments_key: match env::var("PAYMENTS_ENCRYPTION_KEY").ok().filter(|s| !s.trim().is_empty()) {
                Some(k) => Some(crate::utils::crypto::parse_key(&k)?),
                None => None,
            },
            public_base_url: env::var("PUBLIC_BASE_URL").ok().map(|s| s.trim().trim_end_matches('/').to_string()).filter(|s| !s.is_empty()),
            password_reset_redirect: env::var("PASSWORD_RESET_REDIRECT_URL").ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
            email_confirm_redirect: env::var("EMAIL_CONFIRM_REDIRECT_URL").ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
            paynow_base_url: env::var("PAYNOW_BASE_URL").unwrap_or_else(|_| "https://www.paynow.co.zw".into()),
            expo_push_url: env::var("EXPO_PUSH_URL").unwrap_or_else(|_| "https://exp.host/--/api/v2/push/send".into()),
            reminders_enabled: env::var("REMINDERS_ENABLED").map(|v| v != "false" && v != "0").unwrap_or(true),
            app_timezone: env::var("APP_TIMEZONE").unwrap_or_else(|_| "Africa/Harare".into()),
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
