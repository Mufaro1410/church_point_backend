//! Membership lifecycle: several serving roles, release and re-join, account deletion with finance kept.
mod common;

use common::db::{A1_ADMIN, A1_MEMBER, FOC_A1, FOC_A2, setup, uid};
use serde_json::json;
use uuid::Uuid;

async fn ensure_category(env: &common::db::Env) {
    sqlx::query("INSERT INTO categories (name, type) VALUES ('Tithe', 'income') ON CONFLICT DO NOTHING").execute(&env.pool).await.unwrap();
    sqlx::query("INSERT INTO currencies (name, rate, symbol) VALUES ('USD', 1, '$') ON CONFLICT DO NOTHING").execute(&env.pool).await.unwrap();
    sqlx::query("INSERT INTO methods (name) VALUES ('Cash') ON CONFLICT DO NOTHING").execute(&env.pool).await.unwrap();
}

async fn today_event(env: &common::db::Env) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO events (title, location, start_time, end_time, date, foc_id, scope, church_id, province_id, qr_code, qr_generated_at, rsvp, attendance)
         SELECT 'Sunday', 'Hall', '09:00'::timetz, '11:00'::timetz, CURRENT_DATE, f.id, 'foc', f.church_id, f.province_id, 'Q', now(), '{}', '{}' FROM foc_profile f WHERE f.id = $1
         RETURNING id")
        .bind(uid(FOC_A1)).fetch_one(&env.pool).await.unwrap()
}

#[tokio::test]
async fn a_member_can_serve_in_several_places_at_once() {
    let Some(env) = setup().await else { return };
    let m = &env.a1_member;
    let ev = today_event(&env).await;
    let path = format!("/m/events/{ev}/serving");
    let (st, r) = env.call(m, "PUT", &path, Some(json!({"servings": [
        {"ministry_id": null, "capacity": "Prayer"}, {"ministry_id": null, "capacity": "Ushering"}, {"ministry_id": null, "capacity": "prayer"}]}))).await;
    assert_eq!(st, 200, "{r}");
    assert_eq!(r["data"]["servings"].as_array().unwrap().len(), 2, "duplicates (any case) collapse");
    // sending a new set replaces the old one
    env.call(m, "PUT", &path, Some(json!({"servings": [{"ministry_id": null, "capacity": "Parking"}]}))).await;
    let caps: Vec<String> = sqlx::query_scalar("SELECT capacity FROM event_serving WHERE event_id = $1 ORDER BY capacity").bind(ev).fetch_all(&env.pool).await.unwrap();
    assert_eq!(caps, vec!["Parking"]);
    // the two body styles cannot be mixed, and more than ten is refused
    assert_eq!(env.call(m, "PUT", &path, Some(json!({"capacity": "A", "servings": []}))).await.0, 422);
    let many: Vec<_> = (0..11).map(|i| json!({"ministry_id": null, "capacity": format!("Role {i}")})).collect();
    assert_eq!(env.call(m, "PUT", &path, Some(json!({"servings": many}))).await.0, 422);
    env.cleanup().await;
}

#[tokio::test]
async fn released_member_keeps_details_and_can_join_another_church() {
    let Some(env) = setup().await else { return };
    let m = &env.a1_member;
    ensure_category(&env).await;
    // old church records: a prayer and a payment
    sqlx::query("INSERT INTO prayers (title, content, profile_id, date, foc_id) VALUES ('P','c',$1,CURRENT_DATE,$2)").bind(uid(A1_MEMBER)).bind(uid(FOC_A1)).execute(&env.pool).await.unwrap();
    sqlx::query("INSERT INTO incomes (foc_id, member_id, amount, exchange_rate, currency_id, method_id, category_id) VALUES ($1,$2,10,1,(SELECT id FROM currencies LIMIT 1),(SELECT id FROM methods LIMIT 1),(SELECT id FROM categories WHERE type = 'income' LIMIT 1))").bind(uid(FOC_A1)).bind(uid(A1_MEMBER)).execute(&env.pool).await.unwrap();
    let name: Option<String> = sqlx::query_scalar("SELECT payer_name FROM incomes WHERE member_id = $1").bind(uid(A1_MEMBER)).fetch_one(&env.pool).await.unwrap();
    assert_eq!(name.as_deref(), Some("Mary Member"), "the payer's name is stored on the money record");

    // the church releases them
    sqlx::query("UPDATE member_profiles SET foc_id = NULL, status = 'approved' WHERE id = $1").bind(uid(A1_MEMBER)).execute(&env.pool).await.unwrap();
    assert_eq!(env.call(m, "GET", "/m/bootstrap", None).await.0, 403, "no church, no church data");
    let (st, me) = env.call(m, "GET", "/m/me", None).await;
    assert_eq!(st, 200, "{me}");
    assert_eq!(me["data"]["has_church"], false);
    assert_eq!(me["data"]["first_name"], "Mary");
    assert!(me["data"]["phone"].is_string(), "details are kept so the form can be pre-filled");

    // the old church still reads its own records, with the author no longer one of theirs
    let (_, p) = env.call(&env.a1_admin, "GET", "/m/bootstrap", None).await;
    let prayers = p["data"]["prayers"].as_array().unwrap();
    assert_eq!(prayers.len(), 1);
    assert_eq!(prayers[0]["author_name"], "Former member");

    // they pick a new church and wait for approval
    assert_eq!(env.call(m, "PUT", "/m/church", Some(json!({"foc_id": uid(FOC_A2)}))).await.0, 200);
    let (status, foc): (String, Uuid) = sqlx::query_as("SELECT status, foc_id FROM member_profiles WHERE id = $1").bind(uid(A1_MEMBER)).fetch_one(&env.pool).await.unwrap();
    assert_eq!((status.as_str(), foc), ("pending", uid(FOC_A2)));
    let h: Vec<(Uuid, Option<String>, bool)> = sqlx::query_as("SELECT foc_id, end_reason, ended_at IS NULL FROM membership_history WHERE member_id = $1 ORDER BY started_at, ended_at NULLS LAST")
        .bind(uid(A1_MEMBER)).fetch_all(&env.pool).await.unwrap();
    assert_eq!(h.len(), 2);
    assert_eq!((h[0].0, h[0].1.as_deref(), h[0].2), (uid(FOC_A1), Some("released"), false));
    assert_eq!((h[1].0, h[1].2), (uid(FOC_A2), true));
    // the old church's money record is untouched
    let (foc, n): (Uuid, i64) = sqlx::query_as("SELECT foc_id, count(*) OVER () FROM incomes WHERE member_id = $1").bind(uid(A1_MEMBER)).fetch_one(&env.pool).await.unwrap();
    assert_eq!((foc, n), (uid(FOC_A1), 1));
    env.cleanup().await;
}

#[tokio::test]
async fn deleting_an_account_wipes_the_person_but_keeps_the_books() {
    let Some(env) = setup().await else { return };
    let m = &env.a1_member;
    let me = uid(A1_MEMBER);
    ensure_category(&env).await;
    sqlx::query("INSERT INTO incomes (foc_id, member_id, amount, exchange_rate, currency_id, method_id, category_id) VALUES ($1,$2,25,1,(SELECT id FROM currencies LIMIT 1),(SELECT id FROM methods LIMIT 1),(SELECT id FROM categories WHERE type = 'income' LIMIT 1))").bind(uid(FOC_A1)).bind(me).execute(&env.pool).await.unwrap();
    sqlx::query("INSERT INTO prayers (title, content, profile_id, date, foc_id) VALUES ('P','private',$1,CURRENT_DATE,$2)").bind(me).bind(uid(FOC_A1)).execute(&env.pool).await.unwrap();
    sqlx::query("INSERT INTO testimonies (profile_id, foc_id, title, situation, intervention, outcome, author) VALUES ($1,$2,'T','s','i','o','Mary Member')").bind(me).bind(uid(FOC_A1)).execute(&env.pool).await.unwrap();
    sqlx::query("INSERT INTO family_relationships (member_id, related_member_id, relationship_type) VALUES ($1,$2,'other')").bind(me).bind(uid(A1_ADMIN)).execute(&env.pool).await.unwrap();

    // a reason is required, and admins must hand over first
    assert_eq!(env.call(m, "POST", "/m/account/delete", Some(json!({"reason": " "}))).await.0, 422);
    assert_eq!(env.call(&env.a1_admin, "POST", "/m/account/delete", Some(json!({"reason": "leaving"}))).await.0, 422);

    let (st, r) = env.call(m, "POST", "/m/account/delete", Some(json!({"reason": "Moving abroad"}))).await;
    assert_eq!(st, 200, "{r}");

    let (first, last, email, phone, foc, uid_col, deleted): (String, String, Option<String>, String, Option<Uuid>, Option<Uuid>, bool) = sqlx::query_as(
        "SELECT first_name, last_name, email, phone, foc_id, user_id, deleted_at IS NOT NULL FROM member_profiles WHERE id = $1")
        .bind(me).fetch_one(&env.pool).await.unwrap();
    assert_eq!((first.as_str(), last.as_str(), email, phone.as_str(), foc, uid_col, deleted), ("Deleted", "member", None, "", None, None, true));
    for (t, col) in [("prayers", "profile_id"), ("testimonies", "profile_id")] {
        let n: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {t} WHERE {col} = $1")).bind(me).fetch_one(&env.pool).await.unwrap();
        assert_eq!(n, 0, "{t} are deleted");
    }
    let fam: i64 = sqlx::query_scalar("SELECT count(*) FROM family_relationships WHERE member_id = $1 OR related_member_id = $1").bind(me).fetch_one(&env.pool).await.unwrap();
    assert_eq!(fam, 0);

    // finance: the row and the payer's name survive
    let (amount, payer, foc): (f64, Option<String>, Uuid) = sqlx::query_as("SELECT amount::float8, payer_name, foc_id FROM incomes WHERE member_id = $1").bind(me).fetch_one(&env.pool).await.unwrap();
    assert_eq!((amount, payer.as_deref(), foc), (25.0, Some("Mary Member"), uid(FOC_A1)));
    // and the finance API still names them
    let (_, inc) = env.call(&env.a1_admin, "GET", "/incomes", None).await;
    assert!(inc.to_string().contains("Mary Member"), "{inc}");

    let (reason, logged_in): (String, Option<Uuid>) = sqlx::query_as("SELECT reason, auth_user_id FROM account_deletions WHERE member_id = $1").bind(me).fetch_one(&env.pool).await.unwrap();
    assert_eq!(reason, "Moving abroad");
    let _ = logged_in; // cleared only when the login was removed (no Supabase in this test)
    let why: Option<String> = sqlx::query_scalar("SELECT end_reason FROM membership_history WHERE member_id = $1").bind(me).fetch_one(&env.pool).await.unwrap();
    assert_eq!(why.as_deref(), Some("deleted"));
    env.cleanup().await;
}

#[tokio::test]
async fn bible_progress_moves_from_day_numbers_to_dates() {
    let Some(env) = setup().await else { return };
    let plan: Uuid = sqlx::query_scalar("INSERT INTO bible_reading_plans (name, year, readings) VALUES ('DBR', 2026, $1) RETURNING id")
        .bind(json!([{"day":1,"date":"2026-01-01","readings":"A"},{"day":2,"date":"2026-01-02","readings":"B"},{"day":365,"date":"2026-12-31","readings":"Z"}]))
        .fetch_one(&env.pool).await.unwrap();
    sqlx::query("INSERT INTO bible_reading_progress (profile_id, plan_id, progress_data) VALUES ($1,$2,$3)")
        .bind(uid(A1_MEMBER)).bind(plan)
        .bind(json!({"1":{"completed":true},"365":{"completed":true},"20270102":{"completed":true}}))
        .execute(&env.pool).await.unwrap();
    // the migration is safe to run again: already-converted keys stay as they are
    for _ in 0..2 {
        sqlx::raw_sql(include_str!("../migrations/20261120120000_bible_progress_by_date.sql")).execute(&env.pool).await.unwrap();
    }
    let data: serde_json::Value = sqlx::query_scalar("SELECT progress_data FROM bible_reading_progress WHERE plan_id = $1").bind(plan).fetch_one(&env.pool).await.unwrap();
    let mut keys: Vec<_> = data.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, vec!["20260101", "20261231", "20270102"], "{data}");
    assert_eq!(data["20260101"]["completed"], true);
    env.cleanup().await;
}
