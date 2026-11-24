//! Postgres-backed test environment. Skipped unless TEST_DATABASE_URL is set (use a THROWAWAY server:
//! `docker compose up -d testdb`, then
//! `TEST_DATABASE_URL=postgres://postgres:postgres@localhost:55432/postgres cargo test`).
//! The connecting role must be a superuser: it creates databases and the Supabase-like roles.
//!
//! How it works: a template database is built once (cached by content hash):
//!   baseline_infra.sql (superuser)  ->  baseline.sql (as the non-superuser `app_admin`, like Supabase's `postgres`)
//!   ->  the `http` extension is removed (the manual dashboard step)  ->  migrations/  ->  seed data.
//! Every test gets its own copy (CREATE DATABASE ... TEMPLATE), so tests are isolated and fast.
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    str::FromStr,
};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use churchpoint_backend::{build_app, config::Settings, state::AppState};
use jsonwebtoken::{EncodingKey, Header, encode};
use serde_json::{Value, json};
use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use tokio::sync::OnceCell;
use tower::ServiceExt;
use uuid::Uuid;

pub const SUPABASE_URL: &str = "https://proj.supabase.co";
const SECRET: &str = "test-secret";
const INFRA: &str = include_str!("../fixtures/baseline_infra.sql");
const BASELINE: &str = include_str!("../fixtures/baseline.sql");
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// On Supabase the `http` extension's functions are owned by another role, so migrations cannot revoke
/// access to them: the extension has to be disabled by hand. This simulates that step.
const DISABLE_HTTP_EXTENSION: &str = r#"
do $$ declare f record; begin
  for f in select p.oid::regprocedure as sig from pg_proc p
           where p.pronamespace = 'public'::regnamespace
             and p.proname in ('http','http_get','http_post','http_put','http_patch','http_delete','http_head',
                               'http_header','http_list_curlopt','http_reset_curlopt','http_set_curlopt',
                               'urlencode','bytea_to_text','text_to_bytea')
  loop execute format('drop function %s', f.sig); end loop;
end $$;"#;

pub fn uid(n: u32) -> Uuid {
    Uuid::from_u128(n as u128)
}

#[derive(Clone)]
pub struct Actor {
    pub token: String,
    pub user_id: Uuid,
    pub member_id: Uuid,
    pub foc_id: Uuid,
}

// ── seed layout ──
// Church A (province P1): FOC A1 and FOC A2.   Church B: FOC B1 (no province).
pub const CHURCH_A: u32 = 1;
pub const CHURCH_B: u32 = 2;
pub const PROVINCE_1: u32 = 3;
pub const FOC_A1: u32 = 11;
pub const FOC_A2: u32 = 12;
pub const FOC_B1: u32 = 13;
// members (member id = n, auth user id = n + 100)
pub const A1_ADMIN: u32 = 21;
pub const A1_ADMIN2: u32 = 22;
pub const A1_MEMBER: u32 = 23;
pub const A2_ADMIN: u32 = 24;
pub const B1_ADMIN: u32 = 25;

pub fn token_for(user_id: Uuid) -> String {
    let claims = json!({ "sub": user_id, "aud": "authenticated", "exp": chrono::Utc::now().timestamp() + 3600 });
    encode(&Header::default(), &claims, &EncodingKey::from_secret(SECRET.as_bytes())).unwrap()
}

fn actor(member: u32, foc: u32) -> Actor {
    let user_id = uid(member + 100);
    Actor { token: token_for(user_id), user_id, member_id: uid(member), foc_id: uid(foc) }
}

async fn connect_to(url: &str, db: &str, as_app_admin: bool) -> PgPool {
    let opts = PgConnectOptions::from_str(url).expect("TEST_DATABASE_URL").database(db);
    let mut builder = PgPoolOptions::new().max_connections(5);
    if as_app_admin {
        builder = builder.after_connect(|conn, _| {
            Box::pin(async move {
                sqlx::query("SET ROLE app_admin").execute(&mut *conn).await?;
                Ok(())
            })
        });
    }
    builder.connect_with(opts).await.expect("connect to test database")
}

pub async fn seed(pool: &PgPool) {
    sqlx::query("INSERT INTO church(id, name) VALUES ($1,'Church A'),($2,'Church B')")
        .bind(uid(CHURCH_A)).bind(uid(CHURCH_B)).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO province(id, church_id, name) VALUES ($1,$2,'P1')")
        .bind(uid(PROVINCE_1)).bind(uid(CHURCH_A)).execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO foc_profile(id, name, address, email, church_id, province_id) VALUES
         ($1,'A1','addr','a1@x.com',$4,$5), ($2,'A2','addr','a2@x.com',$4,$5), ($3,'B1','addr','b1@x.com',$6,NULL)")
        .bind(uid(FOC_A1)).bind(uid(FOC_A2)).bind(uid(FOC_B1))
        .bind(uid(CHURCH_A)).bind(uid(PROVINCE_1)).bind(uid(CHURCH_B))
        .execute(pool).await.unwrap();

    for (n, foc, role, first, last) in [
        (A1_ADMIN, FOC_A1, "admin", "Ada", "Admin"),
        (A1_ADMIN2, FOC_A1, "admin", "Lee", "Leader"),
        (A1_MEMBER, FOC_A1, "member", "Mary", "Member"),
        (A2_ADMIN, FOC_A2, "admin", "Abe", "Other"),
        (B1_ADMIN, FOC_B1, "admin", "Bob", "Bee"),
    ] {
        sqlx::query("INSERT INTO auth.users(id, email) VALUES ($1, $2)")
            .bind(uid(n + 100)).bind(format!("{first}@x.com")).execute(pool).await.unwrap();
        sqlx::query(
            "INSERT INTO member_profiles(id, user_id, foc_id, email, role, first_name, last_name, gender, phone, status)
             VALUES ($1,$2,$3,$4,$5,$6,$7,'F','0777000000','approved')")
            .bind(uid(n)).bind(uid(n + 100)).bind(uid(foc)).bind(format!("{first}@x.com"))
            .bind(role).bind(first).bind(last)
            .execute(pool).await.unwrap();
    }
}

struct Templates {
    base: String,
    migrated: String,
}
static TEMPLATES: OnceCell<Templates> = OnceCell::const_new();

fn content_key() -> String {
    let mut h = DefaultHasher::new();
    INFRA.hash(&mut h);
    BASELINE.hash(&mut h);
    include_str!("db.rs").hash(&mut h); // any harness change (seed data included) rebuilds the template
    for m in MIGRATOR.iter() {
        m.version.hash(&mut h);
        m.sql.hash(&mut h);
    }
    format!("{:x}", h.finish())
}

async fn build_one(url: &str, admin: &PgPool, name: &str, with_migrations: bool) {
    let building = format!("{name}_building");
    sqlx::query(&format!("DROP DATABASE IF EXISTS {building} WITH (FORCE)")).execute(admin).await.unwrap();
    sqlx::query(&format!("CREATE DATABASE {building}")).execute(admin).await.unwrap();

    let su = connect_to(url, &building, false).await;
    sqlx::raw_sql(INFRA).execute(&su).await.expect("baseline_infra.sql");
    let app = connect_to(url, &building, true).await;
    sqlx::raw_sql(BASELINE).execute(&app).await.expect("baseline.sql");

    if with_migrations {
        sqlx::raw_sql(DISABLE_HTTP_EXTENSION).execute(&su).await.expect("disable http");
        MIGRATOR.run(&app).await.expect("migrations");
        seed(&su).await;
    }
    su.close().await;
    app.close().await;
    // build under a temporary name and rename, so a crashed build never leaves a half-made template
    // a pooled connection can outlive close(); end any session still on the half-built database, then rename
    sqlx::query("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = $1 AND pid <> pg_backend_pid()")
        .bind(&building).execute(admin).await.unwrap();
    sqlx::query(&format!("ALTER DATABASE {building} RENAME TO {name}")).execute(admin).await.unwrap();
}

async fn templates(url: &str) -> &'static Templates {
    let url = url.to_string();
    TEMPLATES
        .get_or_init(|| async move {
            let key = content_key();
            let (base, migrated) = (format!("cp_tpl_base_{key}"), format!("cp_tpl_mig_{key}"));
            let admin = PgPoolOptions::new().max_connections(1).connect(&url).await.expect("connect TEST_DATABASE_URL");
            sqlx::query("SELECT pg_advisory_lock(727272)").execute(&admin).await.unwrap();
            for (name, with_migrations) in [(&base, false), (&migrated, true)] {
                let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
                    .bind(name).fetch_one(&admin).await.unwrap();
                if !exists {
                    build_one(&url, &admin, name, with_migrations).await;
                }
            }
            sqlx::query("SELECT pg_advisory_unlock(727272)").execute(&admin).await.unwrap();
            Templates { base, migrated }
        })
        .await
}

/// Creates a private copy of a template. Serialised, because Postgres dislikes concurrent copies of one template.
async fn copy_template(url: &str, template: &str) -> (PgPool, String) {
    let admin = PgPoolOptions::new().max_connections(1).connect(url).await.unwrap();
    let name = format!("t_{}", Uuid::new_v4().simple());
    sqlx::query("SELECT pg_advisory_lock(727273)").execute(&admin).await.unwrap();
    sqlx::query(&format!("CREATE DATABASE {name} TEMPLATE {template}")).execute(&admin).await.unwrap();
    sqlx::query("SELECT pg_advisory_unlock(727273)").execute(&admin).await.unwrap();
    (admin, name)
}

fn test_url() -> Option<String> {
    match std::env::var("TEST_DATABASE_URL") {
        Ok(u) => Some(u),
        Err(_) => {
            eprintln!("skipped: set TEST_DATABASE_URL to run the database tests");
            None
        }
    }
}

// ───────────────────────── the full application on a seeded, migrated database ─────────────────────────

pub struct Env {
    pub app: Router,
    pub state: churchpoint_backend::state::SharedState,
    /// Superuser connection (the same power the Axum server has: it bypasses Row Level Security).
    pub pool: PgPool,
    admin: PgPool,
    name: String,
    pub a1_admin: Actor,
    pub a1_admin2: Actor,
    pub a1_member: Actor,
    pub a2_admin: Actor,
    pub b1_admin: Actor,
}

pub async fn setup() -> Option<Env> {
    setup_with(None).await
}

/// `mock_supabase`: base url of a fake Supabase (Auth + Storage); also switches on the API keys.
pub async fn setup_with(mock_supabase: Option<String>) -> Option<Env> {
    setup_custom(mock_supabase, None).await
}

/// Also lets a test point the server at a fake Paynow.
pub async fn setup_custom(mock_supabase: Option<String>, paynow: Option<String>) -> Option<Env> {
    setup_full(mock_supabase, paynow, None).await
}

/// Also lets a test point the server at a fake Expo push service.
pub async fn setup_full(mock_supabase: Option<String>, paynow: Option<String>, push: Option<String>) -> Option<Env> {
    let url = test_url()?;
    let t = templates(&url).await;
    let (admin, name) = copy_template(&url, &t.migrated).await;
    let pool = connect_to(&url, &name, false).await;

    let settings = Settings {
        host: "127.0.0.1".into(),
        port: 0,
        database_url: String::new(),
        db_max_connections: 5,
        db_min_connections: 0,
        auth_cache_secs: 0,
        ffmpeg_bin: "ffmpeg".into(),
        payments_key: Some([7u8; 32]),
        password_reset_redirect: None,
        public_base_url: Some("https://example.test".into()),
        paynow_base_url: paynow.unwrap_or_else(|| "https://www.paynow.co.zw".into()),
        expo_push_url: push.unwrap_or_else(|| "http://127.0.0.1:9/push".into()),
        reminders_enabled: false,
        app_timezone: "Africa/Harare".into(),
        audio_max_mb: 5,
        supabase_jwt_secret: SECRET.into(),
        supabase_url: Some(mock_supabase.clone().unwrap_or_else(|| SUPABASE_URL.into())),
        supabase_anon_key: mock_supabase.as_ref().map(|_| "anon-key".to_string()),
        supabase_service_key: mock_supabase.as_ref().map(|_| "service-key".to_string()),
        cors_origins: vec![],
    };
    let state = AppState::new(pool.clone(), settings);
    let app = build_app(state.clone());
    Some(Env {
        app, state, pool, admin, name,
        a1_admin: actor(A1_ADMIN, FOC_A1),
        a1_admin2: actor(A1_ADMIN2, FOC_A1),
        a1_member: actor(A1_MEMBER, FOC_A1),
        a2_admin: actor(A2_ADMIN, FOC_A2),
        b1_admin: actor(B1_ADMIN, FOC_B1),
    })
}

impl Env {
    pub async fn call(&self, who: &Actor, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        let mut req = Request::builder().method(method).uri(format!("/api/v1{uri}"))
            .header(header::AUTHORIZATION, format!("Bearer {}", who.token));
        let body = match body {
            Some(b) => {
                req = req.header(header::CONTENT_TYPE, "application/json");
                Body::from(b.to_string())
            }
            None => Body::empty(),
        };
        let res = self.app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
        let status = res.status();
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    /// A request with no sign-in at all (the public pages).
    pub async fn call_public(&self, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        let mut req = Request::builder().method(method).uri(format!("/api/v1{uri}"));
        let body = match body {
            Some(b) => { req = req.header(header::CONTENT_TYPE, "application/json"); Body::from(b.to_string()) }
            None => Body::empty(),
        };
        let res = self.app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
        let status = res.status();
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    /// Like `call`, with extra request headers; also returns the response headers.
    pub async fn call_with(&self, who: &Actor, method: &str, uri: &str, headers: &[(&str, &str)], body: Option<Value>) -> (StatusCode, Value, axum::http::HeaderMap) {
        let mut req = Request::builder().method(method).uri(format!("/api/v1{uri}"))
            .header(header::AUTHORIZATION, format!("Bearer {}", who.token));
        for (k, v) in headers { req = req.header(*k, *v); }
        let body = match body {
            Some(b) => { req = req.header(header::CONTENT_TYPE, "application/json"); Body::from(b.to_string()) }
            None => Body::empty(),
        };
        let res = self.app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
        let (status, hdrs) = (res.status(), res.headers().clone());
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null), hdrs)
    }

    /// Sends raw bytes (an audio file) as the body.
    /// A public (no login) form post, like Paynow calling back.
    pub async fn raw_post(&self, uri: &str, form: &str) -> (StatusCode, Value) {
        let req = Request::builder().method("POST").uri(format!("/api/v1{uri}"))
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(form.to_string())).unwrap();
        let res = self.app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let b = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&b).unwrap_or(Value::Null))
    }

    pub async fn put_raw(&self, who: &Actor, uri: &str, bytes: Vec<u8>) -> (StatusCode, Value) {
        let req = Request::builder().method("PUT").uri(format!("/api/v1{uri}"))
            .header(header::AUTHORIZATION, format!("Bearer {}", who.token))
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .body(Body::from(bytes)).unwrap();
        let res = self.app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let b = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&b).unwrap_or(Value::Null))
    }

    /// Runs SQL AS a Supabase API role (what a browser/mobile client with the anon key or a login can do),
    /// with `auth.uid()` set to `sub`. Always rolled back. Returns rows affected, or the database's error.
    pub async fn try_as(&self, role: &str, sub: Option<Uuid>, sql: &str) -> Result<u64, String> {
        let mut tx = self.pool.begin().await.unwrap();
        sqlx::query(&format!("SET LOCAL ROLE {role}")).execute(&mut *tx).await.unwrap();
        sqlx::query("SELECT set_config('request.jwt.claim.sub', $1, true)")
            .bind(sub.map(|u| u.to_string()).unwrap_or_default()).execute(&mut *tx).await.unwrap();
        sqlx::query(sql).execute(&mut *tx).await.map(|r| r.rows_affected()).map_err(|e| e.to_string())
    }

    /// `select count(*) from public.<table>` as an API role.
    pub async fn count_as(&self, role: &str, sub: Option<Uuid>, table: &str) -> i64 {
        let mut tx = self.pool.begin().await.unwrap();
        sqlx::query(&format!("SET LOCAL ROLE {role}")).execute(&mut *tx).await.unwrap();
        sqlx::query("SELECT set_config('request.jwt.claim.sub', $1, true)")
            .bind(sub.map(|u| u.to_string()).unwrap_or_default()).execute(&mut *tx).await.unwrap();
        sqlx::query_scalar(&format!("SELECT count(*) FROM public.{table}")).fetch_one(&mut *tx).await.unwrap()
    }

    /// Runs a query that returns one jsonb value, as an API role.
    pub async fn json_as(&self, role: &str, sub: Option<Uuid>, sql: &str) -> Result<Value, String> {
        let mut tx = self.pool.begin().await.unwrap();
        sqlx::query(&format!("SET LOCAL ROLE {role}")).execute(&mut *tx).await.unwrap();
        sqlx::query("SELECT set_config('request.jwt.claim.sub', $1, true)")
            .bind(sub.map(|u| u.to_string()).unwrap_or_default()).execute(&mut *tx).await.unwrap();
        sqlx::query_scalar(sql).fetch_one(&mut *tx).await.map_err(|e| e.to_string())
    }

    pub async fn cleanup(self) {
        self.pool.close().await;
        let _ = sqlx::query(&format!("DROP DATABASE {} WITH (FORCE)", self.name)).execute(&self.admin).await;
    }
}

// ───────────────────────── a bare replica: baseline only, no migrations, no seed ─────────────────────────

pub struct RawDb {
    pub su: PgPool,
    /// Connects as `app_admin` (non-superuser, like Supabase's `postgres`): what `sqlx migrate run` uses.
    pub app: PgPool,
    admin: PgPool,
    name: String,
}

pub async fn raw_baseline() -> Option<RawDb> {
    let url = test_url()?;
    let t = templates(&url).await;
    let (admin, name) = copy_template(&url, &t.base).await;
    Some(RawDb {
        su: connect_to(&url, &name, false).await,
        app: connect_to(&url, &name, true).await,
        admin,
        name,
    })
}

impl RawDb {
    /// Applies migrations whose version is <= `max_version` (and skips ones already applied). Lets a test
    /// create data in the OLD shape, migrate, and check the conversion.
    pub async fn migrate_through(&self, max_version: i64) {
        sqlx::raw_sql(DISABLE_HTTP_EXTENSION).execute(&self.su).await.expect("disable http");
        let subset: Vec<_> = MIGRATOR.iter().filter(|m| m.version <= max_version).cloned().collect();
        let migrator = sqlx::migrate::Migrator { migrations: std::borrow::Cow::Owned(subset), ..sqlx::migrate::Migrator::DEFAULT };
        migrator.run(&self.app).await.expect("migrate");
    }

    pub async fn cleanup(self) {
        self.su.close().await;
        self.app.close().await;
        let _ = sqlx::query(&format!("DROP DATABASE {} WITH (FORCE)", self.name)).execute(&self.admin).await;
    }
}

pub fn code(v: &Value) -> &str {
    v["error"]["code"].as_str().unwrap_or("")
}
