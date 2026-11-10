mod common;

use axum::http::StatusCode;
use common::db::{SUPABASE_URL, code, setup};
use serde_json::{Value, json};

fn find<'a>(list: &'a Value, title: &str) -> &'a Value {
    list["data"].as_array().unwrap().iter().find(|x| x["title"] == title)
        .unwrap_or_else(|| panic!("'{title}' not in {list}"))
}

// ───────────────────────── prayers ─────────────────────────

#[tokio::test]
async fn prayers_hide_anonymous_authors() {
    let Some(env) = setup().await else { return };
    let leader = &env.a1_leader;

    let (s, mine) = env.call(leader, "POST", "/prayers", Some(json!(
        {"title":"Quiet one","content":"private","is_anonymous":true}))).await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(mine["data"]["is_mine"], true);
    assert_eq!(mine["data"]["profile_id"], leader.member_id.to_string()); // author still sees themselves
    env.call(leader, "POST", "/prayers", Some(json!({"title":"Open one","content":"public"}))).await;

    // An admin of the SAME FOC must not learn who wrote the anonymous one.
    let (_, list) = env.call(&env.a1_admin, "GET", "/prayers", None).await;
    let anon = find(&list, "Quiet one");
    assert_eq!(anon["profile_id"], Value::Null);
    assert_eq!(anon["author_name"], "Anonymous");
    assert_eq!(anon["is_mine"], false);
    assert!(!list.to_string().contains(&leader.member_id.to_string()) || find(&list, "Open one")["profile_id"] == leader.member_id.to_string());
    let anon_json = serde_json::to_string(anon).unwrap();
    assert!(!anon_json.contains(&leader.member_id.to_string()), "anonymous prayer leaked the author id");

    let open = find(&list, "Open one");
    assert_eq!(open["author_name"], "Lee Leader");

    // The stored author is the MEMBER id (the React code used the auth user id by mistake).
    let stored: (uuid::Uuid,) = sqlx::query_as("SELECT profile_id FROM prayers WHERE title = 'Quiet one'")
        .fetch_one(&env.pool).await.unwrap();
    assert_eq!(stored.0, leader.member_id);

    // Another FOC sees nothing.
    let (_, other) = env.call(&env.b1_admin, "GET", "/prayers", None).await;
    assert_eq!(other["data"].as_array().unwrap().len(), 0);
    env.cleanup().await;
}

#[tokio::test]
async fn prayer_edit_and_delete_rules() {
    let Some(env) = setup().await else { return };
    let (_, created) = env.call(&env.a1_leader, "POST", "/prayers", Some(json!({"title":"T","content":"C"}))).await;
    let id = created["data"]["id"].as_str().unwrap().to_string();

    // An admin cannot rewrite someone else's prayer, but can remove it (moderation).
    let (s, b) = env.call(&env.a1_admin, "PATCH", &format!("/prayers/{id}"), Some(json!({"title":"hacked"}))).await;
    assert_eq!((s, code(&b)), (StatusCode::FORBIDDEN, "FORBIDDEN"));
    let (s, _) = env.call(&env.a1_leader, "PATCH", &format!("/prayers/{id}"), Some(json!({"title":"mine now"}))).await;
    assert_eq!(s, StatusCode::OK);

    // Other FOCs cannot even tell it exists.
    let (s, b) = env.call(&env.b1_admin, "PATCH", &format!("/prayers/{id}"), Some(json!({"title":"x"}))).await;
    assert_eq!((s, code(&b)), (StatusCode::NOT_FOUND, "PRAYER_NOT_FOUND"));
    let (s, _) = env.call(&env.b1_admin, "DELETE", &format!("/prayers/{id}"), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // A cell leader cannot delete a prayer they don't own.
    let (_, admins) = env.call(&env.a1_admin, "POST", "/prayers", Some(json!({"title":"Admin's","content":"C"}))).await;
    let admin_prayer = admins["data"]["id"].as_str().unwrap();
    let (s, _) = env.call(&env.a1_leader, "DELETE", &format!("/prayers/{admin_prayer}"), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);

    let (s, _) = env.call(&env.a1_admin, "DELETE", &format!("/prayers/{id}"), None).await;
    assert_eq!(s, StatusCode::NO_CONTENT);

    let (s, b) = env.call(&env.a1_leader, "POST", "/prayers", Some(json!({"title":"  ","content":"C"}))).await;
    assert_eq!((s, code(&b)), (StatusCode::UNPROCESSABLE_ENTITY, "VALIDATION_ERROR"));
    env.cleanup().await;
}

// ───────────────────────── testimonies ─────────────────────────

#[tokio::test]
async fn testimony_moderation_flow() {
    let Some(env) = setup().await else { return };
    let leader = &env.a1_leader;

    // Author is stamped from identity even if the client tries to forge it.
    let (s, t) = env.call(leader, "POST", "/testimonies", Some(json!({
        "title":"Healed","situation":"sick","intervention":"prayer","outcome":"well",
        "is_anonymous": true, "author": "Evil Forger", "status": "approved", "profile_id": env.a1_admin.member_id
    }))).await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(t["data"]["status"], "pending");
    assert_eq!(t["data"]["author"], "Lee Leader");
    assert_eq!(t["data"]["profile_id"], leader.member_id.to_string());
    let id = t["data"]["id"].as_str().unwrap().to_string();

    // Length limits give a specific message (the DB would only say "a value is not allowed").
    let long = "x".repeat(151);
    let (s, b) = env.call(leader, "POST", "/testimonies", Some(json!({
        "title":"T","situation": long,"intervention":"i","outcome":"o"}))).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(b["error"]["message"].as_str().unwrap().contains("situation"));

    // Anonymity: the FOC admin sees neither the id nor the real author name.
    let (_, list) = env.call(&env.a1_admin, "GET", "/testimonies", None).await;
    let seen = find(&list, "Healed");
    assert_eq!(seen["profile_id"], Value::Null);
    assert_eq!(seen["author"], "Anonymous");
    assert!(!serde_json::to_string(seen).unwrap().contains("Lee"));

    // Only admins moderate.
    let (s, _) = env.call(leader, "POST", &format!("/testimonies/{id}/approve"), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, b) = env.call(&env.a1_admin, "POST", &format!("/testimonies/{id}/approve"), None).await;
    assert_eq!((s, b["data"]["status"].as_str()), (StatusCode::OK, Some("approved")));
    let (s, _) = env.call(&env.a1_admin, "POST", &format!("/testimonies/{id}/approve"), None).await; // idempotent
    assert_eq!(s, StatusCode::OK);
    let (moderator,): (Option<uuid::Uuid>,) = sqlx::query_as("SELECT moderated_by FROM testimonies").fetch_one(&env.pool).await.unwrap();
    assert_eq!(moderator, Some(env.a1_admin.member_id));

    // Editing an approved testimony sends it back for re-approval and clears the stamp.
    let (s, b) = env.call(leader, "PATCH", &format!("/testimonies/{id}"), Some(json!({"outcome":"even better"}))).await;
    assert_eq!((s, b["data"]["status"].as_str()), (StatusCode::OK, Some("pending")));
    let (moderator,): (Option<uuid::Uuid>,) = sqlx::query_as("SELECT moderated_by FROM testimonies").fetch_one(&env.pool).await.unwrap();
    assert_eq!(moderator, None);

    // Admins can't rewrite a member's testimony.
    let (s, _) = env.call(&env.a1_admin, "PATCH", &format!("/testimonies/{id}"), Some(json!({"title":"x"}))).await;
    assert_eq!(s, StatusCode::FORBIDDEN);

    let (_, b) = env.call(&env.a1_admin, "POST", &format!("/testimonies/{id}/reject"), None).await;
    assert_eq!(b["data"]["status"], "rejected");
    let (_, rejected) = env.call(&env.a1_admin, "GET", "/testimonies?status=rejected", None).await;
    assert_eq!(rejected["data"].as_array().unwrap().len(), 1);
    let (s, b) = env.call(&env.a1_admin, "GET", "/testimonies?status=nonsense", None).await;
    assert_eq!((s, code(&b)), (StatusCode::UNPROCESSABLE_ENTITY, "VALIDATION_ERROR"));

    // Other FOCs cannot moderate it.
    let (s, _) = env.call(&env.b1_admin, "POST", &format!("/testimonies/{id}/approve"), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    env.cleanup().await;
}

// ───────────────────────── announcements ─────────────────────────

#[tokio::test]
async fn announcements_scope_and_ownership() {
    let Some(env) = setup().await else { return };
    let base = |scope: &str, title: &str| json!({"title":title,"content":"c","scope":scope});

    let (s, church) = env.call(&env.a1_admin, "POST", "/announcements", Some(base("church", "Church-wide"))).await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(church["data"]["created_by_foc_id"], env.a1_admin.foc_id.to_string());
    // created_by keeps the existing convention: the AUTH user id (member id + 100 in this fixture).
    let auth_user = uuid::Uuid::from_u128(env.a1_admin.member_id.as_u128() + 100);
    assert_eq!(church["data"]["created_by"], auth_user.to_string());
    let church_id = church["data"]["id"].as_str().unwrap().to_string();
    env.call(&env.a1_admin, "POST", "/announcements", Some(base("foc", "Only A1"))).await;
    env.call(&env.a1_admin, "POST", "/announcements", Some(base("province", "Province-wide"))).await;

    // A2 (same church + province, different FOC) sees church/province ones, not A1's FOC one.
    let (_, list) = env.call(&env.a2_admin, "GET", "/announcements", None).await;
    let titles: Vec<&str> = list["data"].as_array().unwrap().iter().map(|a| a["title"].as_str().unwrap()).collect();
    assert!(titles.contains(&"Church-wide") && titles.contains(&"Province-wide") && !titles.contains(&"Only A1"), "{titles:?}");

    // ...but cannot change or delete what another FOC owns (403 = exists but not yours).
    let (s, b) = env.call(&env.a2_admin, "PATCH", &format!("/announcements/{church_id}"), Some(json!({"title":"hijack"}))).await;
    assert_eq!((s, code(&b)), (StatusCode::FORBIDDEN, "FORBIDDEN"));
    let (s, _) = env.call(&env.a2_admin, "DELETE", &format!("/announcements/{church_id}"), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);

    // Another church sees none of it, and gets 404 (not 403) so existence isn't revealed.
    let (_, other) = env.call(&env.b1_admin, "GET", "/announcements", None).await;
    assert_eq!(other["data"].as_array().unwrap().len(), 0);
    let (s, _) = env.call(&env.b1_admin, "PATCH", &format!("/announcements/{church_id}"), Some(json!({"title":"x"}))).await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // The owner can edit; dates are validated against the stored row.
    let (s, _) = env.call(&env.a1_admin, "PATCH", &format!("/announcements/{church_id}"), Some(json!({"priority":"urgent"}))).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = env.call(&env.a1_admin, "PATCH", &format!("/announcements/{church_id}"), Some(json!({"start_date":"2030-01-10","end_date":"2030-01-01"}))).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    let (s, _) = env.call(&env.a1_admin, "PATCH", &format!("/announcements/{church_id}"), Some(json!({"end_date":"2001-01-01"}))).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "end before the stored start_date must be rejected");
    let (s, _) = env.call(&env.a1_admin, "PATCH", &format!("/announcements/{church_id}"), Some(json!({"priority":"critical"}))).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);

    // Legacy rows (created before the migration: no recorded owner) stay editable by admins who can see them.
    let legacy: (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO announcements(title,content,scope,church_id,start_date)
         SELECT 'Legacy','c','church', id, CURRENT_DATE FROM church WHERE name='A' RETURNING id")
        .fetch_one(&env.pool).await.unwrap();
    let (s, _) = env.call(&env.a2_admin, "PATCH", &format!("/announcements/{}", legacy.0), Some(json!({"title":"Legacy 2"}))).await;
    assert_eq!(s, StatusCode::OK);

    // `active=true` honours is_active and the date window.
    env.call(&env.a1_admin, "POST", "/announcements", Some(json!({"title":"Off","content":"c","scope":"foc","is_active":false}))).await;
    env.call(&env.a1_admin, "POST", "/announcements", Some(json!({"title":"Expired","content":"c","scope":"foc","start_date":"2020-01-01","end_date":"2020-02-01"}))).await;
    let (_, active) = env.call(&env.a1_admin, "GET", "/announcements?active=true", None).await;
    let titles: Vec<&str> = active["data"].as_array().unwrap().iter().map(|a| a["title"].as_str().unwrap()).collect();
    assert!(titles.contains(&"Only A1") && !titles.contains(&"Off") && !titles.contains(&"Expired"), "{titles:?}");

    let (s, _) = env.call(&env.a1_admin, "DELETE", &format!("/announcements/{church_id}"), None).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    env.cleanup().await;
}

// ───────────────────────── sermons ─────────────────────────

#[tokio::test]
async fn sermon_rules() {
    let Some(env) = setup().await else { return };
    let ok = json!({"title":"Grace","preacher":"Pastor","date":"2026-11-01","content":"Long body text...",
        "tags":[" Faith ","faith","Hope"],"youtube_link":"https://youtube.com/watch?v=1"});

    // Only admins manage sermons; leaders may read.
    let (s, _) = env.call(&env.a1_leader, "POST", "/sermons", Some(ok.clone())).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, created) = env.call(&env.a1_admin, "POST", "/sermons", Some(ok.clone())).await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(created["data"]["foc_id"], env.a1_admin.foc_id.to_string());
    assert_eq!(created["data"]["profile_id"], env.a1_admin.member_id.to_string());
    assert_eq!(created["data"]["tags"], json!(["Faith", "Hope"]));
    let id = created["data"]["id"].as_str().unwrap().to_string();

    // Lists omit the large `content`; ?full=true and the single GET include it.
    let (_, list) = env.call(&env.a1_leader, "GET", "/sermons?start=2026-01-01&end=2026-12-31", None).await;
    assert!(find(&list, "Grace").get("content").is_none());
    let (_, full) = env.call(&env.a1_leader, "GET", "/sermons?start=2026-01-01&end=2026-12-31&full=true", None).await;
    assert_eq!(find(&full, "Grace")["content"], "Long body text...");
    let (_, one) = env.call(&env.a1_leader, "GET", &format!("/sermons/{id}"), None).await;
    assert_eq!(one["data"]["content"], "Long body text...");

    // Tenant isolation.
    let (s, b) = env.call(&env.b1_admin, "GET", &format!("/sermons/{id}"), None).await;
    assert_eq!((s, code(&b)), (StatusCode::NOT_FOUND, "SERMON_NOT_FOUND"));
    let (s, _) = env.call(&env.a2_admin, "DELETE", &format!("/sermons/{id}"), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // Links must be https; audio must live in OUR storage bucket.
    let mut bad = ok.clone();
    bad["youtube_link"] = json!("http://youtube.com/x");
    assert_eq!(env.call(&env.a1_admin, "POST", "/sermons", Some(bad)).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let mut bad = ok.clone();
    bad["audio_url"] = json!("https://evil.example/x.opus");
    assert_eq!(env.call(&env.a1_admin, "POST", "/sermons", Some(bad)).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let good_audio = format!("{SUPABASE_URL}/storage/v1/object/public/churchpoint/Sermons/{id}.opus");
    let (s, b) = env.call(&env.a1_admin, "PATCH", &format!("/sermons/{id}"), Some(json!({"audio_url": good_audio}))).await;
    assert_eq!((s, b["data"]["audio_url"].as_str()), (StatusCode::OK, Some(good_audio.as_str())));

    // PATCH: absent = unchanged, null = cleared.
    env.call(&env.a1_admin, "PATCH", &format!("/sermons/{id}"), Some(json!({"scripture":"John 3"}))).await;
    let (_, b) = env.call(&env.a1_admin, "PATCH", &format!("/sermons/{id}"), Some(json!({"scripture":null,"audio_url":null}))).await;
    assert_eq!(b["data"]["scripture"], Value::Null);
    assert_eq!(b["data"]["audio_url"], Value::Null);
    assert_eq!(b["data"]["title"], "Grace");

    // An event may only link to a sermon of its own FOC.
    let ev = json!({"title":"E","date":"2026-11-02","start_time":"09:00","end_time":"10:00","location":"L","scope":"foc","sermon_id": id});
    assert_eq!(env.call(&env.a1_admin, "POST", "/events", Some(ev.clone())).await.0, StatusCode::CREATED);
    assert_eq!(env.call(&env.b1_admin, "POST", "/events", Some(ev)).await.0, StatusCode::UNPROCESSABLE_ENTITY);

    // Deleting a sermon leaves linked events in place (FK is ON DELETE SET NULL).
    assert_eq!(env.call(&env.a1_admin, "DELETE", &format!("/sermons/{id}"), None).await.0, StatusCode::NO_CONTENT);
    env.cleanup().await;
}

// ───────────────────────── events (ownership refactor) ─────────────────────────

#[tokio::test]
async fn church_wide_events_belong_to_the_creating_foc() {
    let Some(env) = setup().await else { return };
    let ev = |scope: &str, title: &str| json!({"title":title,"date":"2026-12-05","start_time":"08:00","end_time":"10:00","location":"L","scope":scope});

    let (s, created) = env.call(&env.a1_admin, "POST", "/events", Some(ev("church", "Conference"))).await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(created["data"]["created_by_foc_id"], env.a1_admin.foc_id.to_string());
    let id = created["data"]["id"].as_str().unwrap().to_string();
    let (_, foc_event) = env.call(&env.a1_admin, "POST", "/events", Some(ev("foc", "A1 only"))).await;
    let foc_event_id = foc_event["data"]["id"].as_str().unwrap().to_string();

    // A2 can see it and take attendance at the door, but cannot edit, delete or re-issue its QR.
    assert_eq!(env.call(&env.a2_admin, "GET", &format!("/events/{id}"), None).await.0, StatusCode::OK);
    for (method, uri, body) in [
        ("PATCH", format!("/events/{id}"), Some(json!({"title":"hijack"}))),
        ("DELETE", format!("/events/{id}"), None),
        ("POST", format!("/events/{id}/qr"), None),
    ] {
        let (s, b) = env.call(&env.a2_admin, method, &uri, body).await;
        assert_eq!((s, code(&b)), (StatusCode::FORBIDDEN, "FORBIDDEN"), "{method} {uri}");
    }
    let (s, _) = env.call(&env.a2_admin, "POST", &format!("/events/{id}/attendance"), Some(json!({"member_id": env.a2_admin.member_id}))).await;
    assert_eq!(s, StatusCode::OK);

    // A FOC-only event is invisible (404) to another FOC.
    let (s, b) = env.call(&env.a2_admin, "PATCH", &format!("/events/{foc_event_id}"), Some(json!({"title":"x"}))).await;
    assert_eq!((s, code(&b)), (StatusCode::NOT_FOUND, "EVENT_NOT_FOUND"));

    // The owner edits freely; a legacy row (no recorded owner) stays editable by admins who see it.
    assert_eq!(env.call(&env.a1_admin, "PATCH", &format!("/events/{id}"), Some(json!({"title":"Conference 2"}))).await.0, StatusCode::OK);
    sqlx::query("UPDATE events SET created_by_foc_id = NULL WHERE id = $1::uuid").bind(&id).execute(&env.pool).await.unwrap();
    assert_eq!(env.call(&env.a2_admin, "PATCH", &format!("/events/{id}"), Some(json!({"title":"Legacy edit"}))).await.0, StatusCode::OK);
    env.cleanup().await;
}

// ───────────────────────── error format ─────────────────────────

#[tokio::test]
async fn bad_input_uses_the_json_error_envelope() {
    let Some(env) = setup().await else { return };
    for uri in ["/events/not-a-uuid", "/sermons/123", "/prayers?start=garbage", "/testimonies/xyz"] {
        let (s, b) = env.call(&env.a1_admin, "GET", uri, None).await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "{uri}");
        assert_eq!(b["success"], false, "{uri}");
        assert_eq!(code(&b), "VALIDATION_ERROR", "{uri}");
    }
    env.cleanup().await;
}
