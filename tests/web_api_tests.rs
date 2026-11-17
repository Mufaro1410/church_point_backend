mod common;

use axum::http::StatusCode;
use common::db::{setup, uid};
use serde_json::{Value, json};

fn id(v: &Value) -> String {
    v["data"]["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn new_web_routes_require_a_token() {
    let app = common::test_app();
    let u = "00000000-0000-0000-0000-000000000001";
    for (m, p) in [
        ("POST", "/members".to_string()), ("POST", format!("/members/{u}/release")), ("POST", format!("/members/{u}/pending")),
        ("GET", format!("/members/{u}/family")), ("DELETE", format!("/family/{u}")),
        ("GET", "/ministries".into()), ("PUT", format!("/ministries/{u}/selected")), ("PUT", format!("/ministries/{u}/leader")),
        ("PATCH", format!("/ministries/{u}")), ("GET", "/courses".into()), ("DELETE", format!("/courses/{u}")),
        ("GET", format!("/events/{u}/serving")), ("DELETE", format!("/event-serving/{u}")),
        ("GET", "/social-events".into()), ("GET", "/bible-plans".into()), ("GET", "/church".into()), ("GET", "/focs".into()),
        ("PATCH", "/foc".into()), ("POST", "/auth/register-foc".into()), ("PATCH", format!("/cell-meetings/{u}")),
    ] {
        let (s, _) = common::request(&app, m, &format!("/api/v1{p}"), None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED, "{m} {p}");
    }
}

#[tokio::test]
async fn members_full_lifecycle() {
    let Some(env) = setup().await else { return };
    let (a1, a2) = (&env.a1_admin, &env.a2_admin);

    // create: forced into my FOC as an approved member; role/foc can't be forged
    let (s, b) = env.call(a1, "POST", "/members", Some(json!({"first_name":"New","last_name":"One","gender":"M","phone":"077","dob":"1990-05-01"}))).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    assert_eq!(b["data"]["role"], "member");
    assert_eq!(b["data"]["foc_id"], a1.foc_id.to_string());
    let mid = id(&b);
    for forged in [json!({"first_name":"x","last_name":"y","gender":"M","phone":"1","role":"admin"}),
                   json!({"first_name":"x","last_name":"y","gender":"M","phone":"1","foc_id":a2.foc_id})] {
        assert_eq!(env.call(a1, "POST", "/members", Some(forged)).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    }
    assert_eq!(env.call(a1, "POST", "/members", Some(json!({"first_name":"x"}))).await.0, StatusCode::UNPROCESSABLE_ENTITY);

    // update arbitrary profile fields, arrays and links
    let (s, g) = env.call(a1, "POST", "/cell-groups", Some(json!({"name":"G"}))).await;
    assert_eq!(s, StatusCode::CREATED);
    let (s, u) = env.call(a1, "PATCH", &format!("/members/{mid}"),
        Some(json!({"profession":"Teacher","course_ids":["c1"],"cell_group_id": id(&g), "dob": null}))).await;
    assert_eq!(s, StatusCode::OK, "{u}");
    assert_eq!(u["data"]["profession"], "Teacher");
    assert_eq!(u["data"]["dob"], Value::Null);
    assert_eq!(env.call(a1, "PATCH", &format!("/members/{mid}"), Some(json!({"role":"admin"}))).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(env.call(a1, "PATCH", &format!("/members/{mid}"), Some(json!({"cell_group_id": uid(999)}))).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.call(a2, "PATCH", &format!("/members/{mid}"), Some(json!({"profession":"x"}))).await.0, StatusCode::NOT_FOUND);

    // search + status filters, other FOC sees nothing of mine
    let (_, l) = env.call(a1, "GET", "/members?search=new", None).await;
    assert_eq!(l["data"]["total"], 1);
    let (_, l) = env.call(a1, "GET", "/members?status=pending", None).await;
    assert_eq!(l["data"]["total"], 0);
    assert_eq!(env.call(a1, "POST", &format!("/members/{mid}/pending"), None).await.0, StatusCode::OK);
    let (_, l) = env.call(a1, "GET", "/members?status=pending", None).await;
    assert_eq!(l["data"]["total"], 1);
    let (_, l) = env.call(a1, "GET", "/members?search=%25", None).await;
    assert_eq!(l["data"]["total"], 0, "LIKE wildcards must be escaped");

    // family: both ends must be in my FOC; no self links; bad type refused
    let other = env.a1_member.member_id;
    let (s, _) = env.call(a1, "POST", &format!("/members/{mid}/family"), Some(json!({"related_member_id": other,"relationship_type":"sibling"}))).await;
    assert_eq!(s, StatusCode::CREATED);
    let (_, f) = env.call(a1, "GET", &format!("/members/{mid}/family"), None).await;
    assert_eq!(f["data"][0]["relationship_type"], "sibling");
    let rel = f["data"][0]["relationship_id"].as_str().unwrap().to_string();
    for body in [json!({"related_member_id": mid,"relationship_type":"sibling"}),
                 json!({"related_member_id": other,"relationship_type":"cousin"}),
                 json!({"related_member_id": a2.member_id,"relationship_type":"sibling"})] {
        let (s, _) = env.call(a1, "POST", &format!("/members/{mid}/family"), Some(body)).await;
        assert!(s == StatusCode::UNPROCESSABLE_ENTITY || s == StatusCode::NOT_FOUND, "{s}");
    }
    assert_eq!(env.call(a2, "DELETE", &format!("/family/{rel}"), None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.call(a1, "DELETE", &format!("/family/{rel}"), None).await.0, StatusCode::NO_CONTENT);

    // release: leaves the FOC; can't release yourself
    assert_eq!(env.call(a1, "POST", &format!("/members/{}/release", a1.member_id), Some(json!({"status":"approved"}))).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(env.call(a1, "POST", &format!("/members/{mid}/release"), Some(json!({"status":"bogus"}))).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(env.call(a1, "POST", &format!("/members/{mid}/release"), Some(json!({"status":"transfer_requested"}))).await.0, StatusCode::NO_CONTENT);
    assert_eq!(env.call(a1, "GET", &format!("/members/{mid}"), None).await.0, StatusCode::NOT_FOUND);
    env.cleanup().await;
}

#[tokio::test]
async fn ministries_courses_and_the_platform_owner() {
    let Some(env) = setup().await else { return };
    let (a1, a2) = (&env.a1_admin, &env.a2_admin);

    let (s, m) = env.call(a1, "POST", "/ministries", Some(json!({"name":"Choir"}))).await;
    assert_eq!(s, StatusCode::CREATED, "{m}");
    let mid = id(&m);
    assert_eq!(env.call(a2, "POST", "/ministries", Some(json!({"name":"choir"}))).await.0, StatusCode::CONFLICT);

    // everyone sees the catalogue, but "selected" is per FOC
    let (_, l1) = env.call(a1, "GET", "/ministries", None).await;
    let (_, l2) = env.call(a2, "GET", "/ministries", None).await;
    assert_eq!(l1["data"][0]["selected"], true);
    assert_eq!(l2["data"][0]["selected"], false);

    // leader needs the ministry enabled and a member of my FOC
    assert_eq!(env.call(a2, "PUT", &format!("/ministries/{mid}/leader"), Some(json!({"leader_id": a2.member_id}))).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(env.call(a1, "PUT", &format!("/ministries/{mid}/leader"), Some(json!({"leader_id": a2.member_id}))).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(env.call(a1, "PUT", &format!("/ministries/{mid}/leader"), Some(json!({"leader_id": env.a1_member.member_id}))).await.0, StatusCode::OK);
    assert_eq!(env.call(a2, "PUT", &format!("/ministries/{mid}/selected"), Some(json!({"selected": true}))).await.0, StatusCode::NO_CONTENT);
    assert_eq!(env.call(a1, "PUT", &format!("/ministries/{mid}/selected"), Some(json!({"selected": false}))).await.0, StatusCode::NO_CONTENT);
    let (_, l1) = env.call(a1, "GET", "/ministries", None).await;
    assert_eq!(l1["data"][0]["selected"], false);
    assert_eq!(l1["data"][0]["leader_id"], Value::Null);

    // FOC admins cannot rename or delete shared entries
    assert_eq!(env.call(a1, "PATCH", &format!("/ministries/{mid}"), Some(json!({"name":"X"}))).await.0, StatusCode::FORBIDDEN);
    assert_eq!(env.call(a1, "DELETE", &format!("/ministries/{mid}"), None).await.0, StatusCode::FORBIDDEN);

    // courses are per church
    let (s, c) = env.call(a1, "POST", "/courses", Some(json!({"name":"Foundations"}))).await;
    assert_eq!(s, StatusCode::CREATED, "{c}");
    assert_eq!(env.call(a1, "POST", "/courses", Some(json!({"name":"foundations"}))).await.0, StatusCode::CONFLICT);
    let (_, cl) = env.call(a2, "GET", "/courses", None).await; // same church (A)
    assert_eq!(cl["data"].as_array().unwrap().len(), 1);
    let (_, cl) = env.call(&env.b1_admin, "GET", "/courses", None).await; // church B
    assert_eq!(cl["data"].as_array().unwrap().len(), 0);

    // the platform owner can rename and delete
    sqlx::query("INSERT INTO auth.users(id, email) VALUES ($1,'su@x.com')").bind(uid(1000)).execute(&env.pool).await.unwrap();
    sqlx::query("INSERT INTO member_profiles(id,user_id,email,role,first_name,last_name,gender,phone,status) VALUES ($1,$2,'su@x.com','super_admin','Su','Per','M','1','approved')")
        .bind(uid(900)).bind(uid(1000)).execute(&env.pool).await.unwrap();
    let mut owner = env.a1_admin.clone();
    owner.token = common::db::token_for(uid(1000));
    let (s, r) = env.call(&owner, "PATCH", &format!("/ministries/{mid}"), Some(json!({"name":"Worship"}))).await;
    assert_eq!(s, StatusCode::OK, "{r}");
    assert_eq!(env.call(&owner, "DELETE", &format!("/courses/{}", id(&c)), None).await.0, StatusCode::NO_CONTENT);
    assert_eq!(env.call(&owner, "DELETE", &format!("/ministries/{mid}"), None).await.0, StatusCode::NO_CONTENT);
    env.cleanup().await;
}

#[tokio::test]
async fn serving_social_events_plans_and_foc_settings() {
    let Some(env) = setup().await else { return };
    let (a1, a2) = (&env.a1_admin, &env.a2_admin);

    // event serving
    let (s, e) = env.call(a1, "POST", "/events", Some(json!({"title":"E","date":"2026-12-05","start_time":"08:00","end_time":"10:00","location":"L","scope":"foc"}))).await;
    assert_eq!(s, StatusCode::CREATED, "{e}");
    let eid = id(&e);
    let (s, sv) = env.call(a1, "POST", &format!("/events/{eid}/serving"), Some(json!({"profile_id": env.a1_member.member_id,"capacity":"usher"}))).await;
    assert_eq!(s, StatusCode::CREATED, "{sv}");
    assert_eq!(env.call(a1, "POST", &format!("/events/{eid}/serving"), Some(json!({"profile_id": env.a1_member.member_id}))).await.0, StatusCode::CONFLICT);
    assert_ne!(env.call(a1, "POST", &format!("/events/{eid}/serving"), Some(json!({"profile_id": a2.member_id}))).await.0, StatusCode::CREATED);
    assert_ne!(env.call(a2, "GET", &format!("/events/{eid}/serving"), None).await.0, StatusCode::OK);
    let (_, l) = env.call(a1, "GET", &format!("/events/{eid}/serving"), None).await;
    assert_eq!(l["data"].as_array().unwrap().len(), 1);
    assert_eq!(env.call(a2, "DELETE", &format!("/event-serving/{}", id(&sv)), None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.call(a1, "DELETE", &format!("/event-serving/{}", id(&sv)), None).await.0, StatusCode::NO_CONTENT);

    // social events: per FOC
    let (s, se) = env.call(a1, "POST", "/social-events", Some(json!({"name":"Braai","type":"Birthday","date":"2026-12-20","time":"14:00"}))).await;
    assert_eq!(s, StatusCode::CREATED, "{se}");
    assert_eq!(env.call(a1, "POST", "/social-events", Some(json!({"name":"x","type":"y","date":"bad"}))).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let (_, l) = env.call(a2, "GET", "/social-events", None).await;
    assert_eq!(l["data"].as_array().unwrap().len(), 0);
    assert_eq!(env.call(a2, "PATCH", &format!("/social-events/{}", id(&se)), Some(json!({"name":"x"}))).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.call(a1, "PATCH", &format!("/social-events/{}", id(&se)), Some(json!({"location":"Park"}))).await.0, StatusCode::OK);
    assert_eq!(env.call(a1, "DELETE", &format!("/social-events/{}", id(&se)), None).await.0, StatusCode::NO_CONTENT);

    // bible plans: shared to read, owner-only to change, soft delete
    assert_eq!(env.call(a1, "POST", "/bible-plans", Some(json!({"name":"P","year":2026,"readings":[]}))).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let (s, p) = env.call(a1, "POST", "/bible-plans", Some(json!({"name":"P","year":2026,"readings":[{"day":1,"ref":"Gen 1"}]}))).await;
    assert_eq!(s, StatusCode::CREATED, "{p}");
    let (_, l) = env.call(a2, "GET", "/bible-plans", None).await;
    assert_eq!(l["data"][0]["is_mine"], false);
    assert_eq!(env.call(a2, "PATCH", &format!("/bible-plans/{}", id(&p)), Some(json!({"name":"hack"}))).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.call(a2, "DELETE", &format!("/bible-plans/{}", id(&p)), None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.call(a1, "PATCH", &format!("/bible-plans/{}", id(&p)), Some(json!({"name":"P2"}))).await.0, StatusCode::OK);
    assert_eq!(env.call(a1, "DELETE", &format!("/bible-plans/{}", id(&p)), None).await.0, StatusCode::NO_CONTENT);
    let (_, l) = env.call(a1, "GET", "/bible-plans", None).await;
    assert_eq!(l["data"].as_array().unwrap().len(), 0);

    // church / focs / settings
    let (_, c) = env.call(a1, "GET", "/church", None).await;
    assert_eq!(c["data"]["name"], "Church A");
    let (_, f) = env.call(a1, "GET", "/focs", None).await;
    assert_eq!(f["data"].as_array().unwrap().len(), 2, "only my church's FOCs");
    assert!(f["data"][0].get("payment_details").is_none());
    let (s, u) = env.call(a1, "PATCH", "/foc", Some(json!({"about":"Hello","payment_details":{"ecocash":"077"}}))).await;
    assert_eq!(s, StatusCode::OK, "{u}");
    assert_eq!(env.call(a1, "PATCH", "/foc", Some(json!({"status":"approved"}))).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(env.call(a1, "PATCH", "/foc", Some(json!({"church_id": uid(2)}))).await.0, StatusCode::UNPROCESSABLE_ENTITY);

    // cell meeting edit
    let (_, g) = env.call(a1, "POST", "/cell-groups", Some(json!({"name":"G"}))).await;
    let (_, m) = env.call(a1, "POST", &format!("/cell-groups/{}/meetings", id(&g)), Some(json!({"date":"2026-11-20"}))).await;
    assert_eq!(env.call(a2, "PATCH", &format!("/cell-meetings/{}", id(&m)), Some(json!({"notes":"x"}))).await.0, StatusCode::NOT_FOUND);
    let (s, u) = env.call(a1, "PATCH", &format!("/cell-meetings/{}", id(&m)), Some(json!({"notes":"Good","attendance":[]}))).await;
    assert_eq!(s, StatusCode::OK, "{u}");
    assert_eq!(u["data"]["notes"], "Good");
    env.cleanup().await;
}

#[tokio::test]
async fn registering_a_foc_starts_pending() {
    let Some(env) = setup().await else { return };
    // a brand-new auth user without a profile
    let new_user = uid(2000);
    sqlx::query("INSERT INTO auth.users(id, email) VALUES ($1,'new@x.com')").bind(new_user).execute(&env.pool).await.unwrap();
    let mut newbie = env.a1_admin.clone();
    newbie.token = common::db::token_for(new_user);

    // before registering: no profile, normal API refuses
    assert_eq!(env.call(&newbie, "GET", "/members", None).await.0, StatusCode::FORBIDDEN);

    let body = json!({"church_data":{"name":"New FOC","address":"1 Road","email":"n@x.com","contact":"077","church_id": uid(1)},
                      "user_data":{"first_name":"Nu","last_name":"Pastor","email":"new@x.com","phone":"077","gender":"M"}});
    let (s, r) = env.call(&newbie, "POST", "/auth/register-foc", Some(body.clone())).await;
    assert_eq!(s, StatusCode::CREATED, "{r}");
    assert_eq!(r["data"]["foc_status"], "pending");
    // registering twice is refused
    assert_eq!(env.call(&newbie, "POST", "/auth/register-foc", Some(body.clone())).await.0, StatusCode::CONFLICT);
    // unknown church refused
    let mut bad = body.clone();
    bad["church_data"]["church_id"] = json!(uid(777));
    let other = uid(2001);
    sqlx::query("INSERT INTO auth.users(id, email) VALUES ($1,'o@x.com')").bind(other).execute(&env.pool).await.unwrap();
    newbie.token = common::db::token_for(other);
    assert_eq!(env.call(&newbie, "POST", "/auth/register-foc", Some(bad)).await.0, StatusCode::UNPROCESSABLE_ENTITY);

    // the new admin exists but FOC data is locked until approved
    newbie.token = common::db::token_for(new_user);
    let (s, me) = env.call(&newbie, "GET", "/auth/me", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(me["data"]["foc_status"], "pending");
    assert_eq!(me["data"]["role"], "admin");
    let (s, b) = env.call(&newbie, "GET", "/members", None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert_eq!(b["error"]["code"], "FOC_NOT_APPROVED");
    env.cleanup().await;
}

#[tokio::test]
async fn roles_profile_serving_list_and_tolerant_times() {
    let Some(env) = setup().await else { return };
    let (a1, a2) = (&env.a1_admin, &env.a2_admin);

    // own full profile, even without FOC data access
    let (s, p) = env.call(a1, "GET", "/auth/profile", None).await;
    assert_eq!((s, p["data"]["first_name"].as_str()), (StatusCode::OK, Some("Ada")));

    // role changes: allowed roles only, not yourself, not other FOCs, login required for admin
    let m = env.a1_member.member_id;
    assert_eq!(env.call(a1, "PUT", &format!("/members/{m}/role"), Some(json!({"role":"pastor"}))).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(env.call(a1, "PUT", &format!("/members/{m}/role"), Some(json!({"role":"super_admin"}))).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(env.call(a1, "PUT", &format!("/members/{}/role", a1.member_id), Some(json!({"role":"member"}))).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(env.call(a2, "PUT", &format!("/members/{m}/role"), Some(json!({"role":"admin"}))).await.0, StatusCode::NOT_FOUND);
    let (s, r) = env.call(a1, "PUT", &format!("/members/{m}/role"), Some(json!({"role":"cell_leader"}))).await;
    assert_eq!((s, r["data"]["role"].as_str()), (StatusCode::OK, Some("cell_leader")));
    let (_, nl) = env.call(a1, "POST", "/members", Some(json!({"first_name":"No","last_name":"Login","gender":"M","phone":"1"}))).await;
    assert_eq!(env.call(a1, "PUT", &format!("/members/{}/role", id(&nl)), Some(json!({"role":"admin"}))).await.0, StatusCode::UNPROCESSABLE_ENTITY);

    // all serving rows of my FOC
    let (_, e) = env.call(a1, "POST", "/events", Some(json!({"title":"E","date":"2026-12-05","start_time":"08:00","end_time":"10:00","location":"L","scope":"foc"}))).await;
    env.call(a1, "POST", &format!("/events/{}/serving", id(&e)), Some(json!({"profile_id": m}))).await;
    assert_eq!(env.call(a1, "GET", "/event-serving", None).await.1["data"].as_array().unwrap().len(), 1);
    assert_eq!(env.call(a2, "GET", "/event-serving", None).await.1["data"].as_array().unwrap().len(), 0);

    // times coming back from the database ("18:30:00") are accepted again
    let (s, g) = env.call(a1, "POST", "/cell-groups", Some(json!({"name":"G","start_time":"18:30:00"}))).await;
    assert_eq!(s, StatusCode::CREATED, "{g}");
    let (s, se) = env.call(a1, "POST", "/social-events", Some(json!({"name":"x","type":"Other","date":"2026-12-20","time":"14:00:00"}))).await;
    assert_eq!(s, StatusCode::CREATED, "{se}");
    env.cleanup().await;
}
