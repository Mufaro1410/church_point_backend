//! A church picks its own province from its group's list; its parent church cannot be changed from the app.
mod common;

use common::db::setup;
use serde_json::json;

#[tokio::test]
async fn province_is_self_service_but_parent_church_is_not() {
    let Some(env) = setup().await else { return };
    let a = &env.a1_admin;

    // the seeded group has a province, so it is listed; the group's own provinces are the only choices
    let prov: uuid::Uuid = sqlx::query_scalar("SELECT id FROM province WHERE name = 'P1'").fetch_one(&env.pool).await.unwrap();
    let other: uuid::Uuid = sqlx::query_scalar("INSERT INTO province(church_id, name) SELECT id, 'Elsewhere' FROM church WHERE name = 'Church B' RETURNING id")
        .fetch_one(&env.pool).await.unwrap();

    let (st, b) = env.call(a, "PATCH", "/foc", Some(json!({"province_id": null}))).await;
    assert_eq!(st, 200, "{b}");
    assert!(b["data"]["province_id"].is_null());
    let (st, b) = env.call(a, "PATCH", "/foc", Some(json!({"province_id": prov, "about": "hello"}))).await;
    assert_eq!(st, 200, "{b}");
    assert_eq!(b["data"]["province_id"], prov.to_string());
    assert_eq!(b["data"]["about"], "hello");

    // another group's province is refused
    assert_eq!(env.call(a, "PATCH", "/foc", Some(json!({"province_id": other}))).await.0, 422);

    // church_id in the body is ignored
    let before: uuid::Uuid = sqlx::query_scalar("SELECT church_id FROM foc_profile WHERE name = 'A1'").fetch_one(&env.pool).await.unwrap();
    let b_church: uuid::Uuid = sqlx::query_scalar("SELECT id FROM church WHERE name = 'Church B'").fetch_one(&env.pool).await.unwrap();
    let (st, _) = env.call(a, "PATCH", "/foc", Some(json!({"church_id": b_church, "about": "x"}))).await;
    assert_eq!(st, 200);
    let after: uuid::Uuid = sqlx::query_scalar("SELECT church_id FROM foc_profile WHERE name = 'A1'").fetch_one(&env.pool).await.unwrap();
    assert_eq!(before, after);

}
