//! The member (mobile) API: one-call bootstrap, RSVP / check-in, reactions, profile, family, church choice.
mod common;

use common::db::{Env, FOC_A1, FOC_A2, code, setup, uid, A1_MEMBER};
use serde_json::{Value, json};
use uuid::Uuid;

async fn event(env: &Env, title: &str, day_offset: i32, scope: &str, foc: u32) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO events (title, location, start_time, end_time, date, foc_id, scope, church_id, province_id, qr_code, qr_generated_at, rsvp, attendance)
         SELECT $1, 'Hall', '09:00'::timetz, '11:00'::timetz, CURRENT_DATE + $2, f.id, $3, f.church_id, f.province_id, 'QRDATA', now(), '{}', '{}' FROM foc_profile f WHERE f.id = $4
         RETURNING id")
        .bind(title).bind(day_offset).bind(scope).bind(uid(foc)).fetch_one(&env.pool).await.unwrap()
}

#[tokio::test]
async fn bootstrap_shapes_and_privacy() {
    let Some(env) = setup().await else { return };
    let m = &env.a1_member;
    let today = event(&env, "Sunday", 0, "foc", FOC_A1).await;
    event(&env, "Other church", 0, "foc", FOC_A2).await;
    // two testimonies in my church: one approved (someone else's), one pending (someone else's)
    for (t, st) in [("Approved one", "approved"), ("Pending one", "pending")] {
        sqlx::query("INSERT INTO testimonies (profile_id, foc_id, title, situation, intervention, outcome, author, status) VALUES ($1,$2,$3,'s','i','o','Ada Admin',$4)")
            .bind(uid(common::db::A1_ADMIN)).bind(uid(FOC_A1)).bind(t).bind(st).execute(&env.pool).await.unwrap();
    }
    let (st, b) = env.call(m, "GET", "/m/bootstrap", None).await;
    assert_eq!(st, 200, "{b}");
    let d = &b["data"];
    for k in ["foc", "events", "prayers", "sermons", "ministries", "ministry_leaders", "cell_groups", "courses",
              "bible_reading_plans", "bible_reading_progress", "announcements", "testimonies", "incomes", "payments", "birthdays"] {
        assert!(d.get(k).is_some(), "missing {k}");
    }
    let events = d["events"].as_array().unwrap();
    assert_eq!(events.len(), 1, "only my church's events");
    assert_eq!(events[0]["id"], today.to_string());
    assert!(events[0].get("qr_code").is_none() && events[0].get("manual_stats").is_none(), "no QR image or headcount for members");
    let titles: Vec<&str> = d["testimonies"].as_array().unwrap().iter().map(|t| t["title"].as_str().unwrap()).collect();
    assert_eq!(titles, vec!["Approved one"], "others' pending testimonies stay hidden");
    assert_eq!(d["foc"]["name"], "A1");
    env.cleanup().await;
}

#[tokio::test]
async fn rsvp_check_in_and_serving() {
    let Some(env) = setup().await else { return };
    let m = &env.a1_member;
    let today = event(&env, "Today", 0, "foc", FOC_A1).await;
    let far = event(&env, "Next month", 30, "foc", FOC_A1).await;
    let other = event(&env, "Elsewhere", 0, "foc", FOC_A2).await;

    let (st, r) = env.call(m, "PUT", &format!("/m/events/{today}/rsvp"), Some(json!({"status":"going"}))).await;
    assert_eq!(st, 200, "{r}");
    assert_eq!(r["data"][uid(A1_MEMBER).to_string()], "going");
    assert_eq!(env.call(m, "PUT", &format!("/m/events/{today}/rsvp"), Some(json!({"status":"sure"}))).await.0, 422);
    assert_eq!(env.call(m, "PUT", &format!("/m/events/{other}/rsvp"), Some(json!({"status":"going"}))).await.0, 404, "cannot touch another church's event");

    let (st, c) = env.call(m, "POST", &format!("/m/events/{today}/check-in"), None).await;
    assert_eq!(st, 200, "{c}");
    let first = c["data"][uid(A1_MEMBER).to_string()].clone();
    assert!(first.is_string());
    let (_, c2) = env.call(m, "POST", &format!("/m/events/{today}/check-in"), None).await;
    assert_eq!(c2["data"][uid(A1_MEMBER).to_string()], first, "the first check-in time is kept");
    assert_eq!(env.call(m, "POST", &format!("/m/events/{far}/check-in"), None).await.0, 422, "only around the event day");

    let (st, s) = env.call(m, "PUT", &format!("/m/events/{today}/serving"), Some(json!({"ministry_id": null, "capacity": "Usher"}))).await;
    assert_eq!(st, 200, "{s}");
    env.call(m, "PUT", &format!("/m/events/{today}/serving"), Some(json!({"ministry_id": null, "capacity": "Greeter"}))).await;
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM event_serving WHERE event_id = $1").bind(today).fetch_one(&env.pool).await.unwrap();
    assert_eq!(n, 1, "serving is one row per person per event");
    env.cleanup().await;
}

#[tokio::test]
async fn prayers_testimonies_and_reactions() {
    let Some(env) = setup().await else { return };
    let m = &env.a1_member;
    let (st, p) = env.call(m, "POST", "/m/prayers", Some(json!({"title":"Healing","content":"Please pray","is_anonymous":true}))).await;
    assert_eq!(st, 201, "{p}");
    let pid = p["data"]["id"].as_str().unwrap().to_string();

    let (_, r1) = env.call(&env.a1_admin, "POST", &format!("/m/prayers/{pid}/support"), None).await;
    assert_eq!(r1["data"]["reactions"].as_array().unwrap().len(), 1);
    let (_, r2) = env.call(&env.a1_admin, "POST", &format!("/m/prayers/{pid}/support"), None).await;
    assert_eq!(r2["data"]["reactions"].as_array().unwrap().len(), 0, "pressing again removes the reaction");

    // anonymous: another member's view hides the author
    let (_, list) = env.call(&env.a1_admin, "GET", "/prayers", None).await;
    assert!(list["data"][0]["profile_id"].is_null(), "{list}");
    // a stranger from another church gets nothing
    assert_eq!(env.call(&env.a2_admin, "POST", &format!("/m/prayers/{pid}/support"), None).await.0, 404);

    let (st, t) = env.call(m, "POST", "/m/testimonies", Some(json!({"title":"God is good","situation":"s","intervention":"i","outcome":"o"}))).await;
    assert_eq!(st, 201, "{t}");
    let tid = t["data"]["id"].as_str().unwrap().to_string();
    assert_eq!(env.call(&env.a1_admin, "POST", &format!("/m/testimonies/{tid}/react"), None).await.0, 404, "pending testimonies take no reactions");
    env.call(&env.a1_admin, "POST", &format!("/testimonies/{tid}/approve"), None).await;
    let (st, r) = env.call(&env.a1_admin, "POST", &format!("/m/testimonies/{tid}/react"), None).await;
    assert_eq!(st, 200, "{r}");
    assert_eq!(r["data"]["reactions"].as_array().unwrap().len(), 1);
    env.cleanup().await;
}

#[tokio::test]
async fn own_profile_family_progress_and_church() {
    let Some(env) = setup().await else { return };
    let m = &env.a1_member;

    // profile: whitelisted fields only
    let (st, r) = env.call(m, "PATCH", "/m/profile", Some(json!({"profession":"Teacher","address":"1 Main St"}))).await;
    assert_eq!(st, 200, "{r}");
    assert_eq!(r["data"]["profession"], "Teacher");
    for bad in [json!({"role":"admin"}), json!({"status":"approved"}), json!({"foc_id": uid(FOC_A2)})] {
        assert_eq!(env.call(m, "PATCH", "/m/profile", Some(bad)).await.0, 422);
    }

    // family
    let (_, found) = env.call(m, "GET", "/m/members/search?q=ada", None).await;
    assert_eq!(found["data"].as_array().unwrap().len(), 1);
    let ada = found["data"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(env.call(m, "POST", "/m/family", Some(json!({"related_member_id": ada, "relationship_type":"cousin"}))).await.0, 422);
    assert_eq!(env.call(m, "POST", "/m/family", Some(json!({"related_member_id": uid(common::db::A2_ADMIN), "relationship_type":"sibling"}))).await.0, 404, "other church");
    assert_eq!(env.call(m, "POST", "/m/family", Some(json!({"related_member_id": ada, "relationship_type":"sibling"}))).await.0, 201);
    let (_, fam) = env.call(m, "GET", "/m/family", None).await;
    assert_eq!(fam["data"][0]["relationship_type"], "sibling");
    assert_eq!(env.call(m, "DELETE", &format!("/m/family/{ada}"), None).await.0, 200);

    // reading progress merges
    let plan: Uuid = sqlx::query_scalar("INSERT INTO bible_reading_plans (name, year, readings) VALUES ('Plan', 2026, '{}') RETURNING id")
        .fetch_one(&env.pool).await.unwrap();
    env.call(m, "PUT", &format!("/m/bible-progress/{plan}"), Some(json!({"progress": {"1": {"isRead": true}}}))).await;
    let (st, p) = env.call(m, "PUT", &format!("/m/bible-progress/{plan}"), Some(json!({"progress": {"2": {"isRead": true}}}))).await;
    assert_eq!(st, 200, "{p}");
    assert_eq!(p["data"]["progress_data"].as_object().unwrap().len(), 2);

    // transfer request and cancel
    assert_eq!(env.call(m, "POST", "/m/transfer/request", None).await.0, 200);
    assert_eq!(env.call(m, "POST", "/m/transfer/request", None).await.0, 422, "already requested");
    assert_eq!(env.call(m, "POST", "/m/transfer/cancel", None).await.0, 200);

    // push token
    assert_eq!(env.call(m, "PUT", "/m/push-token", Some(json!({"token":"ExponentPushToken[abc]"}))).await.0, 200);
    env.cleanup().await;
}

#[tokio::test]
async fn pending_members_and_church_choice() {
    let Some(env) = setup().await else { return };
    let m = &env.a1_member;
    // released member: no church, waiting
    sqlx::query("UPDATE member_profiles SET foc_id = NULL, status = 'transfer_requested' WHERE id = $1").bind(uid(A1_MEMBER)).execute(&env.pool).await.unwrap();
    assert_eq!(env.call(m, "GET", "/m/bootstrap", None).await.0, 403, "no church yet");
    assert_eq!(env.call(m, "PUT", "/m/church", Some(json!({"foc_id": Uuid::new_v4()}))).await.0, 422, "unknown church");
    let (st, r) = env.call(m, "PUT", "/m/church", Some(json!({"foc_id": uid(FOC_A2)}))).await;
    assert_eq!(st, 200, "{r}");
    let status: String = sqlx::query_scalar("SELECT status FROM member_profiles WHERE id = $1").bind(uid(A1_MEMBER)).fetch_one(&env.pool).await.unwrap();
    assert_eq!(status, "pending");
    let (st, b) = env.call(m, "GET", "/m/bootstrap", None).await;
    assert_eq!((st.as_u16(), code(&b)), (403, "FORBIDDEN"), "pending members wait for approval");
    assert_eq!(env.call(m, "PUT", "/m/church", Some(json!({"foc_id": uid(FOC_A1)}))).await.0, 422, "already in a church");

    // the public church list needs no login and exposes only the basics
    let req = axum::http::Request::builder().uri("/api/v1/public/focs").body(axum::body::Body::empty()).unwrap();
    let res = tower::ServiceExt::oneshot(env.app.clone(), req).await.unwrap();
    assert_eq!(res.status(), 200);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert!(v["data"].as_array().unwrap().len() >= 3);
    assert!(v["data"][0].get("email").is_none());
    env.cleanup().await;
}
