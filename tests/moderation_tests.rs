//! Reporting, blocking, the language filter and the admin queue for member posts.
mod common;

use common::db::{A1_MEMBER, setup, uid};
use serde_json::{Value, json};
use uuid::Uuid;

async fn prayer_ids(env: &common::db::Env, who: &common::db::Actor) -> Vec<String> {
    let (_, b) = env.call(who, "GET", "/m/bootstrap", None).await;
    b["data"]["prayers"].as_array().unwrap().iter().map(|p| p["id"].as_str().unwrap().to_string()).collect()
}

#[tokio::test]
async fn reports_blocks_and_the_queue() {
    let Some(env) = setup().await else { return };
    let (a, m) = (&env.a1_admin, &env.a1_member);

    // the member writes an anonymous prayer; the admin reports it
    let (st, p) = env.call(m, "POST", "/m/prayers", Some(json!({"title": "Pray for me", "content": "Please pray", "is_anonymous": true}))).await;
    assert_eq!(st, 201, "{p}");
    let pid = p["data"]["id"].as_str().unwrap().to_string();
    assert!(prayer_ids(&env, a).await.contains(&pid));

    let (st, _) = env.call(a, "POST", "/m/reports", Some(json!({"kind": "prayer", "content_id": pid, "reason": "Not kind"}))).await;
    assert_eq!(st, 201);
    assert_eq!(env.call(a, "POST", "/m/reports", Some(json!({"kind": "prayer", "content_id": pid}))).await.0, 201, "reporting twice is harmless");
    assert!(!prayer_ids(&env, a).await.contains(&pid), "gone from the reporter's feed straight away");
    assert!(prayer_ids(&env, m).await.contains(&pid), "the author still sees their own post");
    assert_eq!(env.call(m, "POST", "/m/reports", Some(json!({"kind": "prayer", "content_id": pid}))).await.0, 422, "cannot report yourself");
    assert_eq!(env.call(a, "POST", "/m/reports", Some(json!({"kind": "prayer", "content_id": uid(424242)}))).await.0, 404);

    // the queue: admins see who wrote it even though it is anonymous; members cannot see the queue
    let (st, q) = env.call(a, "GET", "/moderation/reports", None).await;
    assert_eq!(st, 200);
    let row = &q["data"].as_array().unwrap()[0];
    assert_eq!((row["reports"].as_i64(), row["is_anonymous"].as_bool()), (Some(1), Some(true)));
    assert!(row["author_name"].is_string());
    assert_eq!(env.call(m, "GET", "/moderation/reports", None).await.0, 403);
    assert_eq!(env.call(a, "GET", "/moderation/reports/count", None).await.1["data"]["open"], 1);

    // remove the post and eject its author from the church
    let (st, _) = env.call(a, "POST", &format!("/moderation/reports/prayer/{pid}"), Some(json!({"action": "remove_and_eject"}))).await;
    assert_eq!(st, 204);
    let (gone, foc): (i64, Option<Uuid>) = (
        sqlx::query_scalar("SELECT count(*) FROM prayers WHERE id = $1").bind(Uuid::parse_str(&pid).unwrap()).fetch_one(&env.pool).await.unwrap(),
        sqlx::query_scalar("SELECT foc_id FROM member_profiles WHERE id = $1").bind(uid(A1_MEMBER)).fetch_one(&env.pool).await.unwrap(),
    );
    assert_eq!((gone, foc), (0, None));
    assert_eq!(env.call(a, "GET", "/moderation/reports/count", None).await.1["data"]["open"], 0);
    env.cleanup().await;
}

#[tokio::test]
async fn blocking_hides_an_author_without_naming_them_and_the_filter_rejects_abuse() {
    let Some(env) = setup().await else { return };
    let (a, m) = (&env.a1_admin, &env.a1_member);

    let (_, p) = env.call(m, "POST", "/m/prayers", Some(json!({"title": "Hello", "content": "Pray for exams", "is_anonymous": true}))).await;
    let pid = p["data"]["id"].as_str().unwrap().to_string();
    let (st, _) = env.call(a, "POST", "/m/blocks", Some(json!({"kind": "prayer", "content_id": pid}))).await;
    assert_eq!(st, 201);
    assert!(!prayer_ids(&env, a).await.contains(&pid));
    let (_, b) = env.call(a, "GET", "/m/blocks", None).await;
    let row = &b["data"].as_array().unwrap()[0];
    assert_eq!((row["anonymous"].as_bool(), row["name"].is_null()), (Some(true), true), "blocking an anonymous post must not reveal the author");

    let id = row["id"].as_str().unwrap().to_string();
    assert_eq!(env.call(m, "DELETE", &format!("/m/blocks/{id}"), None).await.0, 204, "someone else's block is not theirs to remove");
    assert_eq!(env.call(a, "GET", "/m/blocks", None).await.1["data"].as_array().unwrap().len(), 1);
    assert_eq!(env.call(a, "DELETE", &format!("/m/blocks/{id}"), None).await.0, 204);
    assert!(prayer_ids(&env, a).await.contains(&pid), "unblocking brings the post back");
    assert_eq!(env.call(m, "POST", "/m/blocks", Some(json!({"member_id": uid(A1_MEMBER)}))).await.0, 422, "cannot block yourself");

    // language filter on create and edit, for prayers and testimonies
    let (st, r) = env.call(m, "POST", "/m/prayers", Some(json!({"title": "You are a b1tch", "content": "x", "is_anonymous": false}))).await;
    assert_eq!(st, 422, "{r}");
    assert_eq!(env.call(m, "PATCH", &format!("/m/prayers/{pid}"), Some(json!({"content": "what the f.u.c.k"}))).await.0, 422);
    let t = json!({"title": "Great", "situation": "Bad shit happened", "intervention": "God", "outcome": "Healed", "is_anonymous": false});
    assert_eq!(env.call(m, "POST", "/m/testimonies", Some(t)).await.0, 422);
    env.cleanup().await;
}

#[tokio::test]
async fn terms_are_recorded_once() {
    let Some(env) = setup().await else { return };
    let m = &env.a1_member;
    assert!(env.call(m, "GET", "/m/me", None).await.1["data"]["terms_accepted_at"].is_null());
    assert_eq!(env.call(m, "POST", "/m/terms/accept", Some(json!({}))).await.0, 200);
    let first = env.call(m, "GET", "/m/me", None).await.1["data"]["terms_accepted_at"].clone();
    assert!(first.is_string());
    env.call(m, "POST", "/m/terms/accept", Some(json!({}))).await;
    assert_eq!(env.call(m, "GET", "/m/me", None).await.1["data"]["terms_accepted_at"], first, "the first acceptance date stays");
    let _: Value = first;
    env.cleanup().await;
}

#[tokio::test]
async fn leaders_only_posts_stay_out_of_the_feed() {
    let Some(env) = setup().await else { return };
    let (a, m) = (&env.a1_admin, &env.a1_member);

    // a leader's own leaders-only post: the member must not see it; the author and other leaders do
    let (st, p) = env.call(a, "POST", "/m/prayers", Some(json!({"title": "Private", "content": "For the elders", "is_anonymous": true}))).await;
    assert_eq!(st, 201, "{p}");
    let pid = p["data"]["id"].as_str().unwrap().to_string();
    assert!(prayer_ids(&env, a).await.contains(&pid));
    assert!(prayer_ids(&env, &env.a1_admin2).await.contains(&pid));
    assert!(!prayer_ids(&env, m).await.contains(&pid), "ordinary members never see leaders-only posts");

    // a member's own leaders-only post is visible to them and to the leaders
    let (_, q) = env.call(m, "POST", "/m/prayers", Some(json!({"title": "Quiet", "content": "Please keep private", "is_anonymous": true}))).await;
    let qid = q["data"]["id"].as_str().unwrap().to_string();
    assert!(prayer_ids(&env, m).await.contains(&qid));
    assert!(prayer_ids(&env, a).await.contains(&qid));
    env.cleanup().await;
}

#[tokio::test]
async fn children_under_13_need_a_guardian_to_agree() {
    let Some(env) = setup().await else { return };
    let m = &env.a1_member;
    sqlx::query("UPDATE member_profiles SET dob = current_date - interval '9 years' WHERE id = $1").bind(uid(A1_MEMBER)).execute(&env.pool).await.unwrap();
    assert_eq!(env.call(m, "GET", "/m/me", None).await.1["data"]["needs_guardian"], true);
    assert_eq!(env.call(m, "POST", "/m/terms/accept", Some(json!({}))).await.0, 422);
    assert_eq!(env.call(m, "POST", "/m/terms/accept", Some(json!({"guardian_name": "A"}))).await.0, 422);
    assert_eq!(env.call(m, "POST", "/m/terms/accept", Some(json!({"guardian_name": "Mrs Moyo"}))).await.0, 200);
    let me = env.call(m, "GET", "/m/me", None).await.1;
    assert_eq!((me["data"]["needs_guardian"].as_bool(), me["data"]["terms_accepted_at"].is_string()), (Some(false), true));
    let name: Option<String> = sqlx::query_scalar("SELECT guardian_name FROM member_profiles WHERE id = $1").bind(uid(A1_MEMBER)).fetch_one(&env.pool).await.unwrap();
    assert_eq!(name.as_deref(), Some("Mrs Moyo"));

    // someone 13 or older is not asked for a guardian
    sqlx::query("UPDATE member_profiles SET dob = current_date - interval '15 years', guardian_name = NULL, guardian_consent_at = NULL, terms_accepted_at = NULL WHERE id = $1").bind(uid(A1_MEMBER)).execute(&env.pool).await.unwrap();
    assert_eq!(env.call(m, "GET", "/m/me", None).await.1["data"]["needs_guardian"], false);
    assert_eq!(env.call(m, "POST", "/m/terms/accept", Some(json!({}))).await.0, 200);
    env.cleanup().await;
}
