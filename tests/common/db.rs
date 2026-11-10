//! Postgres-backed test environment. Skipped unless TEST_DATABASE_URL is set, e.g.
//!   TEST_DATABASE_URL=postgres://postgres:pw@localhost:5432/postgres cargo test --test db_tests
//! Each test gets its own throwaway schema, so tests never touch real data and never collide.
use std::str::FromStr;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use churchpoint_backend::{build_app, config::Settings, state::AppState};
use jsonwebtoken::{EncodingKey, Header, encode};
use serde_json::{Value, json};
use sqlx::{PgPool, postgres::{PgConnectOptions, PgPoolOptions}};
use tower::ServiceExt;
use uuid::Uuid;

pub const SUPABASE_URL: &str = "https://proj.supabase.co";
const SECRET: &str = "test-secret";

#[derive(Clone)]
pub struct Actor {
    pub token: String,
    pub member_id: Uuid,
    pub foc_id: Uuid,
}

pub struct Env {
    pub app: Router,
    pub pool: PgPool,
    admin_pool: PgPool,
    schema: String,
    /// Church A / FOC A1 (province P1): an admin and a cell leader
    pub a1_admin: Actor,
    pub a1_leader: Actor,
    /// Church A / FOC A2 (same province): an admin
    pub a2_admin: Actor,
    /// Church B / FOC B1: an admin
    pub b1_admin: Actor,
}

fn token_for(user_id: Uuid) -> String {
    let claims = json!({ "sub": user_id, "aud": "authenticated", "exp": chrono::Utc::now().timestamp() + 3600 });
    encode(&Header::default(), &claims, &EncodingKey::from_secret(SECRET.as_bytes())).unwrap()
}

pub async fn setup() -> Option<Env> {
    let Ok(url) = std::env::var("TEST_DATABASE_URL") else {
        eprintln!("skipped: set TEST_DATABASE_URL to run the database tests");
        return None;
    };
    let schema = format!("t_{}", Uuid::new_v4().simple());

    let admin_pool = PgPool::connect(&url).await.expect("connect TEST_DATABASE_URL");
    sqlx::query(&format!("CREATE SCHEMA {schema}")).execute(&admin_pool).await.unwrap();

    let opts = PgConnectOptions::from_str(&url).unwrap().options([("search_path", schema.as_str())]);
    let pool = PgPoolOptions::new().max_connections(5).connect_with(opts).await.unwrap();

    sqlx::raw_sql(include_str!("../fixtures/schema.sql")).execute(&pool).await.expect("fixture");
    // The real migration, so the tests also prove the migration SQL is valid. It targets
    // `public.*` in production; here the tables live in the throwaway schema, so drop the prefix.
    let migration = include_str!("../../sql/001_content_migration.sql").replace("public.", "");
    sqlx::raw_sql(&migration).execute(&pool).await.expect("migration");

    let id = |n: u8| Uuid::from_u128(n as u128);
    let (church_a, church_b, prov1) = (id(1), id(2), id(3));
    let (foc_a1, foc_a2, foc_b1) = (id(11), id(12), id(13));
    sqlx::query("INSERT INTO church(id,name) VALUES ($1,'A'),($2,'B')").bind(church_a).bind(church_b).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO province(id,church_id,name) VALUES ($1,$2,'P1')").bind(prov1).bind(church_a).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO foc_profile(id,church_id,province_id,name) VALUES ($1,$2,$3,'A1'),($4,$2,$3,'A2'),($5,$6,NULL,'B1')")
        .bind(foc_a1).bind(church_a).bind(prov1).bind(foc_a2).bind(foc_b1).bind(church_b)
        .execute(&pool).await.unwrap();

    let mut actors = Vec::new();
    for (n, foc, role, first, last) in [
        (21u8, foc_a1, "admin", "Ada", "Admin"),
        (22, foc_a1, "cell_leader", "Lee", "Leader"),
        (23, foc_a2, "admin", "Abe", "Other"),
        (24, foc_b1, "admin", "Bob", "Bee"),
    ] {
        let member = id(n);
        let user = id(n + 100);
        sqlx::query("INSERT INTO member_profiles(id,user_id,foc_id,email,role,first_name,last_name,status) VALUES ($1,$2,$3,$4,$5,$6,$7,'approved')")
            .bind(member).bind(user).bind(foc).bind(format!("{first}@x.com")).bind(role).bind(first).bind(last)
            .execute(&pool).await.unwrap();
        actors.push(Actor { token: token_for(user), member_id: member, foc_id: foc });
    }

    let settings = Settings {
        host: "127.0.0.1".into(),
        port: 0,
        database_url: String::new(),
        db_max_connections: 5,
        supabase_jwt_secret: SECRET.into(),
        supabase_url: Some(SUPABASE_URL.into()),
        cors_origins: vec![],
    };
    let app = build_app(AppState::new(pool.clone(), settings));
    let mut it = actors.into_iter();
    Some(Env {
        app, pool, admin_pool, schema,
        a1_admin: it.next().unwrap(), a1_leader: it.next().unwrap(),
        a2_admin: it.next().unwrap(), b1_admin: it.next().unwrap(),
    })
}

impl Env {
    pub async fn call(&self, who: &Actor, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        let mut req = Request::builder().method(method).uri(format!("/api/v1{uri}"))
            .header(header::AUTHORIZATION, format!("Bearer {}", who.token));
        let body = match body {
            Some(b) => { req = req.header(header::CONTENT_TYPE, "application/json"); Body::from(b.to_string()) }
            None => Body::empty(),
        };
        let res = self.app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
        let status = res.status();
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    pub async fn cleanup(self) {
        self.pool.close().await;
        let _ = sqlx::query(&format!("DROP SCHEMA {} CASCADE", self.schema)).execute(&self.admin_pool).await;
    }
}

pub fn code(v: &Value) -> &str {
    v["error"]["code"].as_str().unwrap_or("")
}
