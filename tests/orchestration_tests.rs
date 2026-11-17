//! Axum as the orchestrator: login/sign-up through Supabase Auth, audio through Supabase Storage.
//! Supabase is replaced by a tiny local fake so nothing leaves the machine.
mod common;

use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, head, post},
};
use common::db::{setup_with, uid, Env};
use serde_json::{Value, json};

#[derive(Default)]
struct Fake {
    signups: Vec<String>,
    deleted_users: Vec<String>,
    uploaded: Vec<String>,
    removed: Vec<String>,
    last_service_key_ok: bool,
}
type Shared = Arc<Mutex<Fake>>;

fn session(user: &str, email: &str) -> Value {
    json!({"access_token":"AT","refresh_token":"RT","expires_in":3600,"expires_at":9999999999i64,
           "user":{"id":user,"email":email,"secret_field":"must not leak"}})
}

async fn token(Query(q): Query<std::collections::HashMap<String, String>>, Json(b): Json<Value>) -> (StatusCode, Json<Value>) {
    let ok = match q.get("grant_type").map(String::as_str) {
        Some("password") => b["email"] == "ada@x.com" && b["password"] == "goodpass1",
        Some("refresh_token") => b["refresh_token"] == "RT",
        _ => false,
    };
    if ok {
        (StatusCode::OK, Json(session(&uid(121).to_string(), "ada@x.com")))
    } else {
        (StatusCode::BAD_REQUEST, Json(json!({"error":"invalid_grant"})))
    }
}

async fn signup(State(f): State<Shared>, Json(b): Json<Value>) -> Json<Value> {
    let email = b["email"].as_str().unwrap().to_string();
    f.lock().unwrap().signups.push(email.clone());
    Json(match email.as_str() {
        "hidden@x.com" => json!({"id": uid(5000), "identities": []}),             // Supabase's "already registered"
        "dup@x.com" => json!({"id": uid(121), "identities": [{}]}),                // collides with an existing profile
        "instant@x.com" => {
            let mut s = session(&uid(6000).to_string(), &email);
            s["user"]["identities"] = json!([{}]);
            s
        }
        _ => json!({"id": uid(7000), "identities": [{}]}),
    })
}

async fn delete_user(State(f): State<Shared>, Path(id): Path<String>) -> StatusCode {
    f.lock().unwrap().deleted_users.push(id);
    StatusCode::NO_CONTENT
}

async fn sign(State(f): State<Shared>, Path(path): Path<String>, h: HeaderMap) -> Json<Value> {
    f.lock().unwrap().last_service_key_ok = h.get("authorization").is_some_and(|v| v == "Bearer service-key");
    Json(json!({"url": format!("/object/upload/sign/{path}?token=t0k3n")}))
}

async fn exists(State(f): State<Shared>, Path(path): Path<String>) -> StatusCode {
    let rel = path.strip_prefix("churchpoint/").unwrap_or(&path).to_string();
    if f.lock().unwrap().uploaded.contains(&rel) { StatusCode::OK } else { StatusCode::NOT_FOUND }
}

async fn remove(State(f): State<Shared>, Path(path): Path<String>) -> StatusCode {
    let rel = path.strip_prefix("churchpoint/").unwrap_or(&path).to_string();
    let mut g = f.lock().unwrap();
    g.uploaded.retain(|p| *p != rel);
    g.removed.push(rel);
    StatusCode::OK
}

async fn list() -> Json<Value> {
    Json(json!([{"name":"logo.png"},{"name":"notes.txt"},{"name":"Banner.SVG"}]))
}

async fn start_fake() -> (String, Shared) {
    // reqwest honours proxy variables; local traffic must bypass them.
    unsafe { std::env::set_var("NO_PROXY", "127.0.0.1,localhost") };
    let fake = Shared::default();
    let app = Router::new()
        .route("/auth/v1/token", post(token))
        .route("/auth/v1/signup", post(signup))
        .route("/auth/v1/logout", post(|| async { StatusCode::NO_CONTENT }))
        .route("/auth/v1/recover", post(|| async { Json(json!({})) }))
        .route("/auth/v1/admin/users/{id}", delete(delete_user))
        .route("/storage/v1/object/upload/sign/{*path}", post(sign))
        .route("/storage/v1/object/public/{*path}", head(exists))
        .route("/storage/v1/object/list/{bucket}", post(list))
        .route("/storage/v1/object/{*path}", delete(remove))
        .with_state(fake.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, fake)
}

async fn post_public(env: &Env, uri: &str, body: Value) -> (StatusCode, Value) {
    use axum::{body::{Body, to_bytes}, http::{Request, header}};
    use tower::ServiceExt;
    let req = Request::builder().method("POST").uri(format!("/api/v1{uri}"))
        .header(header::CONTENT_TYPE, "application/json").body(Body::from(body.to_string())).unwrap();
    let res = env.app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

fn id(v: &Value) -> String {
    v["data"]["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn login_needs_supabase_keys_and_no_token() {
    let app = common::test_app();
    let (s, b) = common::request(&app, "POST", "/api/v1/auth/login", None).await;
    assert_ne!(s, StatusCode::UNAUTHORIZED, "public route: {b}");
    for p in ["/auth/logout"] {
        assert_eq!(common::request(&app, "POST", &format!("/api/v1{p}"), None).await.0, StatusCode::UNAUTHORIZED);
    }
}

#[tokio::test]
async fn login_refresh_logout_and_forgot_password() {
    let (url, _fake) = start_fake().await;
    let Some(env) = setup_with(Some(url)).await else { return };

    let (s, b) = post_public(&env, "/auth/login", json!({"email":"  ADA@x.com ","password":"goodpass1"})).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["data"]["session"]["access_token"], "AT");
    assert_eq!(b["data"]["me"]["role"], "admin");
    assert_eq!(b["data"]["me"]["foc_status"], "approved");
    assert!(b["data"]["user"].get("secret_field").is_none(), "raw Supabase user must not be passed through");

    for bad in [json!({"email":"ada@x.com","password":"nope"}), json!({"email":"not-an-email","password":"x"}), json!({"email":"ada@x.com","password":""})] {
        let (s, b) = post_public(&env, "/auth/login", bad).await;
        assert_eq!((s, b["error"]["code"].as_str()), (StatusCode::UNAUTHORIZED, Some("INVALID_CREDENTIALS")));
    }

    assert_eq!(post_public(&env, "/auth/refresh", json!({"refresh_token":"RT"})).await.0, StatusCode::OK);
    assert_eq!(post_public(&env, "/auth/refresh", json!({"refresh_token":"x"})).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(post_public(&env, "/auth/forgot-password", json!({"email":"nobody@x.com"})).await.0, StatusCode::NO_CONTENT);
    assert_eq!(post_public(&env, "/auth/forgot-password", json!({"email":"garbage"})).await.0, StatusCode::NO_CONTENT);
    assert_eq!(env.call(&env.a1_admin, "POST", "/auth/logout", None).await.0, StatusCode::NO_CONTENT);
    env.cleanup().await;
}

#[tokio::test]
async fn register_creates_login_and_foc_together_without_ghost_accounts() {
    let (url, fake) = start_fake().await;
    let Some(env) = setup_with(Some(url)).await else { return };
    let body = |email: &str, church: u32| json!({
        "email": email, "password": "longenough1",
        "church_data": {"name":"  New   FOC ","address":"1 Road","email":"f@x.com","contact":"077","church_id": uid(church)},
        "user_data": {"first_name":"Nu","last_name":"Pastor","email":"ignored@x.com","phone":"077","gender":"M"}});

    // all checks happen BEFORE the login is created
    for (b, want) in [
        (body("a@x.com", 777), StatusCode::UNPROCESSABLE_ENTITY),   // unknown church
        (body("ada@x.com", 1), StatusCode::CONFLICT),               // email already has a profile
        (body("bad", 1), StatusCode::UNPROCESSABLE_ENTITY),
        ({ let mut b = body("c@x.com", 1); b["password"] = json!("short"); b }, StatusCode::UNPROCESSABLE_ENTITY),
    ] {
        assert_eq!(post_public(&env, "/auth/register", b).await.0, want);
    }
    assert!(fake.lock().unwrap().signups.is_empty(), "no login may be created for a request that is going to fail");

    // (the real Supabase creates the auth.users row; the fake does not, so do it here)
    for (n, e) in [(7000, "new@x.com"), (6000, "instant@x.com")] {
        sqlx::query("INSERT INTO auth.users(id, email) VALUES ($1,$2)").bind(uid(n)).bind(e).execute(&env.pool).await.unwrap();
    }

    // happy path: confirmation required, FOC pending, name tidied, email taken from the account
    let (s, r) = post_public(&env, "/auth/register", body("New@X.com", 1)).await;
    assert_eq!(s, StatusCode::CREATED, "{r}");
    assert_eq!(r["data"]["confirmation_required"], true);
    assert_eq!(r["data"]["foc"]["foc_status"], "pending");
    let (name, email): (String, String) = sqlx::query_as("SELECT f.name, m.email FROM foc_profile f JOIN member_profiles m ON m.foc_id = f.id WHERE m.user_id = $1")
        .bind(uid(7000)).fetch_one(&env.pool).await.unwrap();
    assert_eq!((name.as_str(), email.as_str()), ("New FOC", "new@x.com"));

    // project without email confirmation: session returned straight away
    let (_, r) = post_public(&env, "/auth/register", body("instant@x.com", 1)).await;
    assert_eq!(r["data"]["session"]["access_token"], "AT");
    assert_eq!(r["data"]["confirmation_required"], false);

    // Supabase's "already registered" answer, and a failure AFTER sign-up (login is removed again)
    assert_eq!(post_public(&env, "/auth/register", body("hidden@x.com", 1)).await.0, StatusCode::CONFLICT);
    let (s, _) = post_public(&env, "/auth/register", body("dup@x.com", 1)).await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert!(fake.lock().unwrap().deleted_users.contains(&uid(121).to_string()), "ghost login must be deleted");
    env.cleanup().await;
}

#[tokio::test]
async fn sermon_audio_goes_through_axum_and_text_is_formatted() {
    let (url, fake) = start_fake().await;
    let Some(env) = setup_with(Some(url.clone())).await else { return };
    let (a1, a2) = (&env.a1_admin, &env.a2_admin);

    // formatted and trimmed before storage
    let messy = json!({"title":"  The   Power\n of\tPrayer ","preacher":" Pastor\u{200b} Joe ","date":"2026-11-15",
                       "scripture":"  John   3:16 ","summary":"\r\n Short   \r\n\r\n\r\n\r\nsummary \r\n",
                       "content":"\r\nPoint 1   \r\n\r\n\r\n\r\n\tsub\u{0007}point\r\n\r\n"});
    let (s, b) = env.call(a1, "POST", "/sermons", Some(messy)).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    let d = &b["data"];
    assert_eq!(d["title"], "The Power of Prayer");
    assert_eq!(d["preacher"], "Pastor Joe");
    assert_eq!(d["scripture"], "John 3:16");
    assert_eq!(d["summary"], "Short\n\nsummary");
    assert_eq!(d["content"], "Point 1\n\n    subpoint");
    let sid = id(&b);
    let (_, u) = env.call(a1, "PATCH", &format!("/sermons/{sid}"), Some(json!({"title":"  New   title  "}))).await;
    assert_eq!(u["data"]["title"], "New title");

    // audio can't be supplied at creation, nor pointed at someone else's file
    let (s, _) = env.call(a1, "POST", "/sermons", Some(json!({"title":"t","preacher":"p","date":"2026-11-15","content":"c","audio_url":"https://x.co/a.opus"}))).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    let other = format!("{url}/storage/v1/object/public/churchpoint/Sermons/{}.opus", uid(1));
    assert_eq!(env.call(a1, "PATCH", &format!("/sermons/{sid}"), Some(json!({"audio_url": other}))).await.0, StatusCode::UNPROCESSABLE_ENTITY);

    // step 1: signed upload url (only for your own sermon)
    assert_eq!(env.call(a2, "POST", &format!("/sermons/{sid}/audio/upload-url"), None).await.0, StatusCode::NOT_FOUND);
    let (s, up) = env.call(a1, "POST", &format!("/sermons/{sid}/audio/upload-url"), None).await;
    assert_eq!(s, StatusCode::OK, "{up}");
    assert!(up["data"]["upload_url"].as_str().unwrap().ends_with("token=t0k3n"));
    assert!(fake.lock().unwrap().last_service_key_ok, "storage must be called with the server's key");

    // step 2: confirm only works once the file really exists
    assert_eq!(env.call(a1, "POST", &format!("/sermons/{sid}/audio/confirm"), None).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    fake.lock().unwrap().uploaded.push(format!("Sermons/{sid}.opus"));
    let (s, c) = env.call(a1, "POST", &format!("/sermons/{sid}/audio/confirm"), None).await;
    assert_eq!(s, StatusCode::OK, "{c}");
    assert!(c["data"]["audio_url"].as_str().unwrap().ends_with(&format!("Sermons/{sid}.opus")));
    assert_eq!(env.call(a2, "DELETE", &format!("/sermons/{sid}/audio"), None).await.0, StatusCode::NOT_FOUND);

    // removing the audio deletes the file and clears the link; deleting the sermon cleans up too
    let (s, r) = env.call(a1, "DELETE", &format!("/sermons/{sid}/audio"), None).await;
    assert_eq!((s, &r["data"]["audio_url"]), (StatusCode::OK, &Value::Null));
    assert!(fake.lock().unwrap().removed.contains(&format!("Sermons/{sid}.opus")));
    fake.lock().unwrap().removed.clear();
    assert_eq!(env.call(a1, "DELETE", &format!("/sermons/{sid}"), None).await.0, StatusCode::NO_CONTENT);
    assert!(fake.lock().unwrap().removed.contains(&format!("Sermons/{sid}.opus")));

    // logos: images only, as public urls
    let (_, l) = env.call(a1, "GET", "/logos", None).await;
    let l = l["data"].as_array().unwrap();
    assert_eq!(l.len(), 2);
    assert!(l[0].as_str().unwrap().ends_with("/churchpoint/logo.png"));
    env.cleanup().await;
}

#[tokio::test]
async fn member_views_usage_counts_and_event_services() {
    let Some(env) = setup_with(None).await else { return };
    let (a1, a2) = (&env.a1_admin, &env.a2_admin);
    let m = env.a1_member.member_id;

    // event + check-in -> attendance view
    let (_, e) = env.call(a1, "POST", "/events", Some(json!({"title":"Sunday","date":"2026-12-06","start_time":"08:00","end_time":"10:00","location":"Hall","scope":"foc"}))).await;
    let eid = id(&e);
    assert_eq!(env.call(a1, "POST", &format!("/events/{eid}/attendance"), Some(json!({"member_id": m}))).await.0, StatusCode::OK);
    let (s, att) = env.call(a1, "GET", &format!("/members/{m}/attendance"), None).await;
    assert_eq!(s, StatusCode::OK, "{att}");
    assert_eq!(att["data"][0]["eventTitle"], "Sunday");
    assert_eq!(env.call(a2, "GET", &format!("/members/{m}/attendance"), None).await.0, StatusCode::NOT_FOUND);

    // activity: own prayers show, anonymous ones are never attributed
    sqlx::query("INSERT INTO prayers(title, content, is_anonymous, profile_id, foc_id, date) VALUES ('Open','c',false,$1,$2,current_date),('Secret','c',true,$1,$2,current_date)")
        .bind(m).bind(a1.foc_id).execute(&env.pool).await.unwrap();
    let (_, act) = env.call(a1, "GET", &format!("/members/{m}/activity"), None).await;
    let acts = act["data"].as_array().unwrap();
    assert_eq!(acts.len(), 1);
    assert_eq!(acts[0]["description"], "Open");

    // contributions (voided excluded) and usage counts
    assert_eq!(env.call(a1, "GET", &format!("/members/{m}/contributions"), None).await.1["data"].as_array().unwrap().len(), 0);
    let (s, u) = env.call(a1, "GET", &format!("/finance/currencies/{}/usage", uid(999)), None).await;
    assert_eq!((s, &u["data"]["count"]), (StatusCode::OK, &json!(0)));
    for kind in ["categories", "methods"] {
        assert_eq!(env.call(a1, "GET", &format!("/finance/{kind}/{}/usage", uid(999)), None).await.0, StatusCode::OK);
    }

    // event services
    let svc = json!({"member_id": m, "service_role": "  Usher ", "service_domain":"Hospitality"});
    let (s, r) = env.call(a1, "POST", &format!("/events/{eid}/services"), Some(svc.clone())).await;
    assert_eq!(s, StatusCode::CREATED, "{r}");
    assert_eq!(r["data"]["service_role"], "Usher");
    assert_eq!(r["data"]["source"], "manual");
    assert_eq!(r["data"]["marked_by"], a1.member_id.to_string());
    assert_eq!(env.call(a1, "POST", &format!("/events/{eid}/services"), Some(svc)).await.0, StatusCode::CONFLICT);
    assert_eq!(env.call(a1, "POST", &format!("/events/{eid}/services"), Some(json!({"member_id": a2.member_id,"service_role":"x"}))).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.call(a1, "POST", &format!("/events/{eid}/services"), Some(json!({"member_id": m,"service_role":"  "}))).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_ne!(env.call(a2, "GET", &format!("/events/{eid}/services"), None).await.0, StatusCode::OK);
    assert_eq!(env.call(a1, "GET", &format!("/events/{eid}/services"), None).await.1["data"].as_array().unwrap().len(), 1);
    assert_eq!(env.call(a2, "DELETE", &format!("/event-services/{}", id(&r)), None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.call(a1, "DELETE", &format!("/event-services/{}", id(&r)), None).await.0, StatusCode::NO_CONTENT);
    env.cleanup().await;
}
