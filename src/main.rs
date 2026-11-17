use std::str::FromStr;

use churchpoint_backend::{build_app, config::Settings, state::AppState};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "churchpoint_backend=debug,tower_http=debug,info".into()),
        )
        .with(fmt::layer())
        .init();

    let settings = Settings::from_env()?;

    // statement_cache_capacity(0) keeps us compatible with Supabase's transaction-mode
    // pooler (port 6543), which doesn't support prepared statements.
    let options = PgConnectOptions::from_str(&settings.database_url)?.statement_cache_capacity(0);
    let pool = PgPoolOptions::new()
        .max_connections(settings.db_max_connections)
        .min_connections(settings.db_min_connections.min(settings.db_max_connections))
        .acquire_timeout(std::time::Duration::from_secs(15))
        .idle_timeout(std::time::Duration::from_secs(300))
        .max_lifetime(std::time::Duration::from_secs(1800))
        // Skip the extra "are you alive?" round trip on every checkout; with a far-away database that doubled each query's wait.
        .test_before_acquire(false)
        .connect_with(options)
        .await?;

    // NOTE: we deliberately do NOT run sqlx::migrate! here. The schema already exists in
    // Supabase and is managed there. Add migrations only for NEW tables you create via Axum.

    let addr = format!("{}:{}", settings.host, settings.port);
    let state = AppState::new(pool, settings);
    // Old idempotency records are only needed for a short while.
    let cleanup_db = state.db.clone();
    tokio::spawn(async move {
        loop {
            let _ = sqlx::query("DELETE FROM idempotency_keys WHERE created_at < now() - interval '1 day'")
                .execute(&cleanup_db)
                .await;
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        }
    });
    let app = build_app(state);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("listening on http://{addr}");

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
            tracing::info!("shutting down");
        })
        .await?;
    Ok(())
}
