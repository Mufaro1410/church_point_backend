mod common;

use axum::http::StatusCode;
use common::db::{setup, uid};
use serde_json::json;

#[tokio::test]
async fn org_and_platform_routes_require_a_token() {
    let app = common::test_app();
    for (m, u) in [
        ("GET", "/api/v1/cell-groups"), ("POST", "/api/v1/cell-groups"), ("GET", "/api/v1/ministries"),
        ("GET", "/api/v1/dashboard"), ("GET", "/api/v1/platform/focs"),
        ("PUT", "/api/v1/platform/focs/00000000-0000-0000-0000-000000000001/status"),
    ] {
        let (s, b) = common::request(&app, m, u, None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED, "{m} {u}");
        assert_eq!(b["error"]["code"], "MISSING_TOKEN");
    }
}

#[tokio::test]
async fn cell_groups_meetings_and_isolation() {
    let Some(env) = setup().await else { return };
    let (a1, a2) = (&env.a1_admin, &env.a2_admin);

    // validation
    let (s, _) = env.call(a1, "POST", "/cell-groups", Some(json!({"description": "x"}))).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    let (s, _) = env.call(a1, "POST", "/cell-groups", Some(json!({"name":"G","start_time":"25:99"}))).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    // leader from another FOC is refused
    let (s, b) = env.call(a1, "POST", "/cell-groups", Some(json!({"name":"G","leader_id": a2.member_id}))).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(b["error"]["code"], "LEADER_NOT_IN_FOC");

    let (s, g) = env.call(a1, "POST", "/cell-groups",
        Some(json!({"name":"Alpha","meeting_day":"Friday","start_time":"18:30","leader_id": env.a1_member.member_id}))).await;
    assert_eq!(s, StatusCode::CREATED, "{g}");
    let id = g["data"]["id"].as_str().unwrap().to_string();

    // other FOC cannot see/change/delete it
    for (m, body) in [("GET", None), ("PATCH", Some(json!({"name":"hack"}))), ("DELETE", None)] {
        let (s, _) = env.call(a2, m, &format!("/cell-groups/{id}"), body).await;
        assert_eq!(s, StatusCode::NOT_FOUND, "{m}");
    }
    let (_, l) = env.call(a2, "GET", "/cell-groups", None).await;
    assert_eq!(l["data"].as_array().unwrap().len(), 0);

    // meetings
    let (s, _) = env.call(a1, "POST", &format!("/cell-groups/{id}/meetings"), Some(json!({"date":"nope"}))).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    let (s, _) = env.call(a1, "POST", &format!("/cell-groups/{id}/meetings"), Some(json!({"date":"2026-11-20","attendance":"x"}))).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    let (s, _) = env.call(a2, "POST", &format!("/cell-groups/{id}/meetings"), Some(json!({"date":"2026-11-20"}))).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, m) = env.call(a1, "POST", &format!("/cell-groups/{id}/meetings"),
        Some(json!({"date":"2026-11-20","attendance":[{"member_id": env.a1_member.member_id}]}))).await;
    assert_eq!(s, StatusCode::CREATED, "{m}");
    let mid = m["data"]["id"].as_str().unwrap().to_string();
    let (_, ms) = env.call(a1, "GET", &format!("/cell-groups/{id}/meetings"), None).await;
    assert_eq!(ms["data"].as_array().unwrap().len(), 1);
    assert_eq!(env.call(a2, "DELETE", &format!("/cell-meetings/{mid}"), None).await.0, StatusCode::NOT_FOUND);
    let (_, all) = env.call(a1, "GET", "/cell-meetings", None).await;
    assert_eq!(all["data"].as_array().unwrap().len(), 1);
    assert_eq!(env.call(a2, "GET", "/cell-meetings", None).await.1["data"].as_array().unwrap().len(), 0);
    assert_eq!(env.call(a1, "DELETE", &format!("/cell-meetings/{mid}"), None).await.0, StatusCode::NO_CONTENT);

    // dashboard counts only own FOC; members (non-admin) are refused
    let (_, d) = env.call(a1, "GET", "/dashboard", None).await;
    assert_eq!(d["data"]["members_total"], 3);
    assert_eq!(d["data"]["cell_groups"], 1);
    assert_eq!(env.call(&env.a1_member, "GET", "/dashboard", None).await.0, StatusCode::FORBIDDEN);

    assert_eq!(env.call(a1, "DELETE", &format!("/cell-groups/{id}"), None).await.0, StatusCode::NO_CONTENT);
    env.cleanup().await;
}

#[tokio::test]
async fn foc_approval_and_platform_owner() {
    let Some(env) = setup().await else { return };
    let (a1, b1) = (&env.a1_admin, &env.b1_admin);

    // a normal admin cannot use the platform API
    assert_eq!(env.call(a1, "GET", "/platform/focs", None).await.0, StatusCode::FORBIDDEN);

    // make a super admin (no FOC of their own)
    let su = uid(900);
    sqlx::query("INSERT INTO auth.users(id, email) VALUES ($1,'su@x.com')").bind(uid(1000)).execute(&env.pool).await.unwrap();
    sqlx::query("INSERT INTO member_profiles(id,user_id,email,role,first_name,last_name,gender,phone,status) VALUES ($1,$2,'su@x.com','super_admin','Su','Per','M','1','approved')")
        .bind(su).bind(uid(1000)).execute(&env.pool).await.unwrap();
    let mut owner = env.a1_admin.clone();
    owner.user_id = uid(1000);
    owner.token = common::db::token_for(uid(1000));

    // suspend FOC B1 -> its admin is locked out of FOC data
    assert_eq!(env.call(b1, "GET", "/members", None).await.0, StatusCode::OK);
    let (s, b) = env.call(&owner, "PUT", &format!("/platform/focs/{}/status", b1.foc_id), Some(json!({"status":"suspended"}))).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert!(b["data"].get("payment_details").is_none());
    let (s, b) = env.call(b1, "GET", "/members", None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert_eq!(b["error"]["code"], "FOC_NOT_APPROVED");

    // bad status / unknown FOC
    assert_eq!(env.call(&owner, "PUT", &format!("/platform/focs/{}/status", b1.foc_id), Some(json!({"status":"x"}))).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(env.call(&owner, "PUT", &format!("/platform/focs/{}/status", uid(777)), Some(json!({"status":"approved"}))).await.0, StatusCode::NOT_FOUND);

    // re-approve; list filter works
    env.call(&owner, "PUT", &format!("/platform/focs/{}/status", b1.foc_id), Some(json!({"status":"approved"}))).await;
    assert_eq!(env.call(b1, "GET", "/members", None).await.0, StatusCode::OK);
    let (_, l) = env.call(&owner, "GET", "/platform/focs?status=approved", None).await;
    assert_eq!(l["data"].as_array().unwrap().len(), 3);
    env.cleanup().await;
}
