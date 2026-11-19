//! Serving capacities (per church, admin-managed) and push notifications (through a fake Expo).
mod common;

use std::sync::{Arc, Mutex};

use axum::{Json, Router, extract::State, routing::post};
use common::db::{A1_ADMIN, A1_MEMBER, FOC_A1, setup_full, uid};
use serde_json::{Value, json};

type Seen = Arc<Mutex<Vec<Value>>>;

async fn fake_expo() -> (String, Seen) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/push", listener.local_addr().unwrap());
    let seen: Seen = Arc::new(Mutex::new(vec![]));
    let app = Router::new()
        .route("/push", post(|State(s): State<Seen>, Json(b): Json<Vec<Value>>| async move {
            let tickets: Vec<Value> = b.iter().map(|m| {
                if m["to"].as_str().is_some_and(|t| t.contains("dead")) {
                    json!({"status":"error","details":{"error":"DeviceNotRegistered"}})
                } else { json!({"status":"ok"}) }
            }).collect();
            s.lock().unwrap().extend(b);
            Json(json!({ "data": tickets }))
        }))
        .with_state(seen.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, seen)
}

async fn wait_for(seen: &Seen, n: usize) {
    for _ in 0..40 {
        if seen.lock().unwrap().len() >= n { return; }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn capacities_are_per_church_and_admin_only() {
    let Some(env) = setup_full(None, None, None).await else { return };
    let (st, c) = env.call(&env.a1_admin, "POST", "/serving-capacities", Some(json!({"name": "  Prayer   team "}))).await;
    assert_eq!(st, 201, "{c}");
    assert_eq!(c["data"]["name"], "Prayer team", "spaces are tidied");
    let id = c["data"]["id"].as_str().unwrap().to_string();
    assert_eq!(env.call(&env.a1_admin, "POST", "/serving-capacities", Some(json!({"name": "PRAYER TEAM"}))).await.0, 422, "no duplicates (any case)");
    assert_eq!(env.call(&env.a1_admin, "POST", "/serving-capacities", Some(json!({"name": ""}))).await.0, 422);
    assert_eq!(env.call(&env.a1_member, "POST", "/serving-capacities", Some(json!({"name": "Sneaky"}))).await.0, 403, "members cannot manage them");

    // another church's admin neither sees nor changes it
    let (_, other) = env.call(&env.a2_admin, "GET", "/serving-capacities", None).await;
    assert_eq!(other["data"].as_array().unwrap().len(), 0);
    assert_eq!(env.call(&env.a2_admin, "PATCH", &format!("/serving-capacities/{id}"), Some(json!({"name": "Hijack"}))).await.0, 422);
    assert_eq!(env.call(&env.a2_admin, "DELETE", &format!("/serving-capacities/{id}"), None).await.0, 422);

    // members of that church get the list in their one-call bootstrap
    let (_, b) = env.call(&env.a1_member, "GET", "/m/bootstrap", None).await;
    assert_eq!(b["data"]["serving_capacities"][0]["name"], "Prayer team");
    assert_eq!(b["data"]["notification_prefs"], json!({"enabled": true, "announcements": true, "events": true}));

    let (st, r) = env.call(&env.a1_admin, "PATCH", &format!("/serving-capacities/{id}"), Some(json!({"name": "Prayer"}))).await;
    assert_eq!(st, 200, "{r}");
    assert_eq!(env.call(&env.a1_admin, "DELETE", &format!("/serving-capacities/{id}"), None).await.0, 204);
    env.cleanup().await;
}

#[tokio::test]
async fn pushes_respect_switches_and_clean_dead_tokens() {
    let (url, seen) = fake_expo().await;
    let Some(env) = setup_full(None, None, Some(url)).await else { return };
    let m = &env.a1_member;
    let (st, _) = env.call(m, "PUT", "/m/push-token", Some(json!({"token": "ExponentPushToken[member]"}))).await;
    assert_eq!(st, 200);

    // announcement → the member's phone is told
    let ann = json!({"title": "Camp meeting", "content": "Friday night at the hall", "scope": "foc"});
    let (st, r) = env.call(&env.a1_admin, "POST", "/announcements", Some(ann.clone())).await;
    assert_eq!(st, 201, "{r}");
    wait_for(&seen, 1).await;
    {
        let g = seen.lock().unwrap();
        assert_eq!(g.len(), 1);
        assert_eq!(g[0]["to"], "ExponentPushToken[member]");
        assert!(g[0]["title"].as_str().unwrap().contains("Camp meeting"));
        assert_eq!(g[0]["data"]["screen"], "announcements");
    }

    // switch announcements off → nothing more arrives; events still do
    let (st, p) = env.call(m, "PUT", "/m/notifications", Some(json!({"announcements": false}))).await;
    assert_eq!(st, 200, "{p}");
    assert_eq!(p["data"], json!({"enabled": true, "announcements": false, "events": true}));
    env.call(&env.a1_admin, "POST", "/announcements", Some(ann)).await;
    let ev = json!({"title": "Youth night", "date": "2030-01-05", "start_time": "18:00", "end_time": "20:00", "location": "Hall", "scope": "foc"});
    let (st, r) = env.call(&env.a1_admin, "POST", "/events", Some(ev.clone())).await;
    assert_eq!(st, 201, "{r}");
    wait_for(&seen, 2).await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    {
        let g = seen.lock().unwrap();
        assert_eq!(g.len(), 2, "announcement was muted, the event came through");
        assert!(g[1]["title"].as_str().unwrap().contains("Youth night"));
    }

    // master switch off → silence
    env.call(m, "PUT", "/m/notifications", Some(json!({"enabled": false}))).await;
    env.call(&env.a1_admin, "POST", "/events", Some(ev)).await;
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    assert_eq!(seen.lock().unwrap().len(), 2);

    // a token Expo says is dead is forgotten
    sqlx::query("UPDATE member_push_tokens SET token = 'ExponentPushToken[dead]' WHERE member_id = $1").bind(uid(A1_MEMBER)).execute(&env.pool).await.unwrap();
    env.call(m, "PUT", "/m/notifications", Some(json!({"enabled": true, "announcements": true}))).await;
    env.call(&env.a1_admin, "POST", "/announcements", Some(json!({"title": "Again", "content": "x", "scope": "foc"}))).await;
    wait_for(&seen, 3).await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM member_push_tokens WHERE member_id = $1").bind(uid(A1_MEMBER)).fetch_one(&env.pool).await.unwrap();
    assert_eq!(n, 0, "dead token removed");
    assert_eq!(env.call(m, "PUT", "/m/notifications", Some(json!({"nope": true}))).await.0, 422);
    let _ = A1_ADMIN;
    env.cleanup().await;
}

#[tokio::test]
async fn reminders_go_once_to_going_and_maybe() {
    let (url, seen) = fake_expo().await;
    let Some(env) = setup_full(None, None, Some(url)).await else { return };
    let m = &env.a1_member;
    env.call(m, "PUT", "/m/push-token", Some(json!({"token": "ExponentPushToken[member]"}))).await;

    // pick a time zone where it is 12:00 right now, so the "morning of" reminder is due
    let hour: i32 = sqlx::query_scalar("SELECT extract(hour FROM now() AT TIME ZONE 'UTC')::int").fetch_one(&env.pool).await.unwrap();
    let n = hour - 12; // local = utc - n
    let tz = if n == 0 { "Etc/GMT".to_string() } else if n > 0 { format!("Etc/GMT+{n}") } else { format!("Etc/GMT-{}", -n) };

    let id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO events (title, location, start_time, end_time, date, foc_id, scope, church_id, province_id, qr_code, qr_generated_at, rsvp, attendance)
         SELECT 'Sunday service', 'Hall', '09:00'::timetz, '11:00'::timetz, (now() AT TIME ZONE $2)::date, f.id, 'foc', f.church_id, f.province_id, 'Q', now(), '{}', '{}' FROM foc_profile f WHERE f.id = $1
         RETURNING id")
        .bind(uid(FOC_A1)).bind(&tz).fetch_one(&env.pool).await.unwrap();

    // nobody said going yet → the reminder is claimed but nobody is told
    assert_eq!(churchpoint_backend::services::push_service::run_reminders_at(&env.state, &tz).await, 1);
    assert_eq!(seen.lock().unwrap().len(), 0);
    sqlx::query("DELETE FROM event_reminders_sent").execute(&env.pool).await.unwrap();

    env.call(m, "PUT", &format!("/m/events/{id}/rsvp"), Some(json!({"status": "going"}))).await;
    assert_eq!(churchpoint_backend::services::push_service::run_reminders_at(&env.state, &tz).await, 1);
    wait_for(&seen, 1).await;
    assert_eq!(seen.lock().unwrap().len(), 1);
    assert!(seen.lock().unwrap()[0]["title"].as_str().unwrap().contains("Today"));
    // the same reminder never goes twice
    assert_eq!(churchpoint_backend::services::push_service::run_reminders_at(&env.state, &tz).await, 0);
    assert_eq!(seen.lock().unwrap().len(), 1);
    env.cleanup().await;
}
