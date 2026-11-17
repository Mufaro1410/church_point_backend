use std::sync::Arc;

use sqlx::PgPool;

use crate::config::Settings;

pub struct AppState {
    pub db: PgPool,
    pub supabase: crate::clients::supabase::SupabaseClient,
    pub settings: Settings,
}

/// Cheap-to-clone handle passed to handlers via `State<SharedState>`.
pub type SharedState = Arc<AppState>;

impl AppState {
    pub fn new(db: PgPool, settings: Settings) -> SharedState {
        let supabase = crate::clients::supabase::SupabaseClient::new(&settings);
        Arc::new(Self { db, supabase, settings })
    }
}
