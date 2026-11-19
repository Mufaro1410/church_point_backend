use std::sync::Arc;

use sqlx::PgPool;

use crate::config::Settings;

pub struct AppState {
    pub db: PgPool,
    /// Who a token belongs to, remembered for a few seconds (see `auth_cache_secs`).
    pub auth_cache: std::sync::Mutex<std::collections::HashMap<uuid::Uuid, (std::time::Instant, crate::models::auth::AuthProfile)>>,
    pub supabase: crate::clients::supabase::SupabaseClient,
    pub paynow: crate::clients::paynow::PaynowClient,
    pub push: crate::clients::expo_push::ExpoPush,
    pub settings: Settings,
}

/// Cheap-to-clone handle passed to handlers via `State<SharedState>`.
pub type SharedState = Arc<AppState>;

impl AppState {
    /// Call after anything that changes who someone is (role, church status, removal).
    pub fn clear_auth_cache(&self) {
        if let Ok(mut c) = self.auth_cache.lock() {
            c.clear();
        }
    }

    pub fn new(db: PgPool, settings: Settings) -> SharedState {
        let supabase = crate::clients::supabase::SupabaseClient::new(&settings);
        let paynow = crate::clients::paynow::PaynowClient::new(&settings.paynow_base_url);
        let push = crate::clients::expo_push::ExpoPush::new(&settings.expo_push_url);
        Arc::new(Self { db, auth_cache: Default::default(), supabase, paynow, push, settings })
    }
}
