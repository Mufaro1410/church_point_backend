//! Custom roles, permissions, limits by age group, and age groups themselves.
mod common;

use common::db::{A1_MEMBER, FOC_A1, FOC_A2, setup, uid};
use serde_json::{Value, json};
use uuid::Uuid;

async fn role(env: &common::db::Env, name: &str, perms: Value, ages: Value) -> (axum::http::StatusCode, Value) {
    let (st, r) = env.call(&env.a1_admin, "POST", "/roles", Some(json!({"name": name, "permissions": perms, "scope_age_group_ids": ages}))).await;
    (st, r)
}

#[tokio::test]
async fn permissions_are_checked_normalised_and_admin_only_to_manage() {
    let Some(env) = setup().await else { return };
    let a = &env.a1_admin;
    let (st, r) = role(&env, "Clerk", json!(["giving.manage"]), json!([])).await;
    assert_eq!(st, 201, "{r}");
    let perms: Vec<&str> = r["data"]["permissions"].as_array().unwrap().iter().map(|p| p.as_str().unwrap()).collect();
    assert_eq!(perms, vec!["giving.manage", "giving.view"], "manage includes view");
    assert_eq!(role(&env, "clerk", json!(["giving.view"]), json!([])).await.0, 422, "names are unique per church");
    assert_eq!(role(&env, "X", json!(["giving.delete"]), json!([])).await.0, 422, "money cannot be deleted");
    assert_eq!(role(&env, "X", json!(["nothing.view"]), json!([])).await.0, 422);
    assert_eq!(role(&env, "X", json!([]), json!([])).await.0, 422, "at least one permission");
    assert_eq!(role(&env, "X", json!(["members.view"]), json!([uid(9999)])).await.0, 422, "limits must belong to this church");
    let (st, cat) = env.call(a, "GET", "/permissions", None).await;
    assert_eq!(st, 200);
    assert!(cat["data"].as_array().unwrap().iter().any(|x| x["key"] == "giving" && x["actions"] == json!(["view", "manage"])));

    // a member holding a role cannot manage roles or age groups
    let role_id = r["data"]["id"].as_str().unwrap().to_string();
    let (st, _) = env.call(a, "PUT", &format!("/members/{}/roles", uid(A1_MEMBER)), Some(json!({"role_ids": [role_id]}))).await;
    assert_eq!(st, 200);
    let m = &env.a1_member;
    assert_eq!(env.call(m, "GET", "/roles", None).await.0, 403);
    assert_eq!(env.call(m, "POST", "/age-groups", Some(json!({"name":"A","min_age":0,"max_age":5}))).await.0, 403);
    env.cleanup().await;
}

#[tokio::test]
async fn a_role_opens_only_what_it_grants() {
    let Some(env) = setup().await else { return };
    let (a, m) = (&env.a1_admin, &env.a1_member);
    // nothing without a role
    assert_eq!(env.call(m, "GET", "/members", None).await.0, 403);
    assert_eq!(env.call(m, "GET", "/incomes", None).await.0, 403);

    let (_, r) = role(&env, "Giving viewer", json!(["giving.view", "members.view"]), json!([])).await;
    let rid = r["data"]["id"].as_str().unwrap().to_string();
    env.call(a, "PUT", &format!("/members/{}/roles", uid(A1_MEMBER)), Some(json!({"role_ids": [rid]}))).await;
    assert_eq!(env.call(m, "GET", "/members", None).await.0, 200);
    assert_eq!(env.call(m, "GET", "/incomes", None).await.0, 200);
    assert_eq!(env.call(m, "GET", "/expenses", None).await.0, 403, "no expenses permission");
    assert_eq!(env.call(m, "POST", "/incomes", Some(json!({}))).await.0, 403, "view only: cannot record giving");
    assert_eq!(env.call(m, "POST", "/members", Some(json!({}))).await.0, 403, "view only: cannot add members");
    assert_eq!(env.call(m, "GET", "/events", None).await.0, 403);
    assert_eq!(env.call(m, "GET", "/church", None).await.0, 200, "lookups work for any staff role");

    // taking the role away closes everything again straight away
    env.call(a, "PUT", &format!("/members/{}/roles", uid(A1_MEMBER)), Some(json!({"role_ids": []}))).await;
    assert_eq!(env.call(m, "GET", "/members", None).await.0, 403);

    // roles of another church cannot be assigned
    let other: Uuid = sqlx::query_scalar("INSERT INTO church_roles (foc_id, name, permissions) VALUES ($1,'Other',$2) RETURNING id")
        .bind(uid(FOC_A2)).bind(vec!["members.view".to_string()]).fetch_one(&env.pool).await.unwrap();
    assert_eq!(env.call(a, "PUT", &format!("/members/{}/roles", uid(A1_MEMBER)), Some(json!({"role_ids": [other]}))).await.0, 422);
    env.cleanup().await;
}

#[tokio::test]
async fn age_groups_cannot_overlap_and_can_limit_what_a_role_sees() {
    let Some(env) = setup().await else { return };
    let (a, m) = (&env.a1_admin, &env.a1_member);
    let mk = |n: &str, lo: i32, hi: Option<i32>| json!({"name": n, "min_age": lo, "max_age": hi});
    let (st, kids) = env.call(a, "POST", "/age-groups", Some(mk("Children", 0, Some(12)))).await;
    assert_eq!(st, 201, "{kids}");
    let (_, youth) = env.call(a, "POST", "/age-groups", Some(mk("Arena", 13, Some(18)))).await;
    env.call(a, "POST", "/age-groups", Some(mk("Seniors", 60, None))).await;
    assert_eq!(env.call(a, "POST", "/age-groups", Some(mk("Mixed", 10, Some(20)))).await.0, 422, "overlaps Children");
    assert_eq!(env.call(a, "POST", "/age-groups", Some(mk("Old", 50, Some(70)))).await.0, 422, "overlaps Seniors");
    assert_eq!(env.call(a, "POST", "/age-groups", Some(mk("Bad", 30, Some(20)))).await.0, 422);
    assert_eq!(env.call(a, "POST", "/age-groups", Some(mk("Adults", 19, Some(59)))).await.0, 201, "a gap filled exactly is fine");
    // the same ages can change when it is that group itself
    let kid_id = kids["data"]["id"].as_str().unwrap();
    assert_eq!(env.call(a, "PATCH", &format!("/age-groups/{kid_id}"), Some(mk("Kids", 0, Some(12)))).await.0, 200);

    for (first, years) in [("Tiny", 6), ("Teen", 15), ("Grown", 40)] {
        sqlx::query("INSERT INTO member_profiles (foc_id, first_name, last_name, gender, phone, status, dob) VALUES ($1,$2,'T','M','0770','approved', current_date - ($3 || ' years')::interval)")
            .bind(uid(FOC_A1)).bind(first).bind(years.to_string()).execute(&env.pool).await.unwrap();
    }
    let names = |r: &Value| -> Vec<String> { r["data"]["items"].as_array().unwrap().iter().map(|x| x["first_name"].as_str().unwrap().to_string()).collect() };
    let youth_id = youth["data"]["id"].as_str().unwrap().to_string();
    let (_, l) = env.call(a, "GET", &format!("/members?age_group_id={youth_id}"), None).await;
    assert_eq!(names(&l), vec!["Teen"], "filter by age group");

    // a role limited to Arena sees only Arena members
    let (_, r) = role(&env, "Arena leader", json!(["members.manage"]), json!([youth_id])).await;
    let rid = r["data"]["id"].as_str().unwrap().to_string();
    env.call(a, "PUT", &format!("/members/{}/roles", uid(A1_MEMBER)), Some(json!({"role_ids": [rid]}))).await;
    let (st, l) = env.call(m, "GET", "/members", None).await;
    assert_eq!(st, 200);
    assert_eq!(names(&l), vec!["Teen"], "{l}");
    assert_eq!(l["data"]["total"], 1);
    let id_of = |first: &str| { let first = first.to_string(); let pool = env.pool.clone(); async move { sqlx::query_scalar::<_, Uuid>("SELECT id FROM member_profiles WHERE first_name = $1").bind(first).fetch_one(&pool).await.unwrap() } };
    let (teen, tiny) = (id_of("Teen").await, id_of("Tiny").await);
    assert_eq!(env.call(m, "GET", &format!("/members/{teen}"), None).await.0, 200);
    assert_eq!(env.call(m, "GET", &format!("/members/{tiny}"), None).await.0, 403, "outside the limit");
    assert_eq!(env.call(m, "PATCH", &format!("/members/{tiny}"), Some(json!({"first_name":"X"}))).await.0, 403);
    assert_eq!(env.call(m, "POST", "/members", Some(json!({"first_name":"N","last_name":"N","gender":"M","phone":"0771"}))).await.0, 403, "a limited role cannot add members");

    // deleting an age group removes it from role limits and the list
    env.call(a, "DELETE", &format!("/age-groups/{youth_id}"), None).await;
    let scope: Vec<Uuid> = sqlx::query_scalar("SELECT unnest(scope_age_group_ids) FROM church_roles WHERE id = $1::uuid").bind(&rid).fetch_all(&env.pool).await.unwrap();
    assert!(scope.is_empty());
    let (_, ag) = env.call(a, "GET", "/age-groups", None).await;
    assert!(ag["data"].as_array().unwrap().iter().all(|g| g["name"] != "Arena"));
    assert!(ag["data"].as_array().unwrap().iter().any(|g| g["name"] == "Kids" && g["member_count"] == 1));
    env.cleanup().await;
}
