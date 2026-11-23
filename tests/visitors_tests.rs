//! Visitors, the follow-up log and the "stopped attending" list.
mod common;

use common::db::{A1_MEMBER, CHURCH_A, FOC_A1, setup, uid};
use serde_json::json;

#[tokio::test]
async fn visitors_follow_up_and_absent() {
    let Some(env) = setup().await else { return };
    let (a, m, other) = (&env.a1_admin, &env.a1_member, &env.a2_admin);

    // a plain member has no access; validation holds
    assert_eq!(env.call(m, "GET", "/visitors", None).await.0, 403);
    assert_eq!(env.call(a, "POST", "/visitors", Some(json!({"full_name": "  "}))).await.0, 422);
    assert_eq!(env.call(a, "POST", "/visitors", Some(json!({"full_name": "X", "status": "bogus"}))).await.0, 422);

    let (st, v) = env.call(a, "POST", "/visitors", Some(json!({"full_name": "Tendai Moyo", "phone": "0771", "how_heard": "Friend"}))).await;
    assert_eq!(st, 201, "{v}");
    let id = v["data"]["id"].as_str().unwrap().to_string();
    assert_eq!(v["data"]["status"], "new");

    // another church cannot see or touch it
    assert_eq!(env.call(other, "GET", &format!("/visitors/{id}"), None).await.0, 422);
    assert_eq!(env.call(other, "GET", "/visitors", None).await.1["data"].as_array().unwrap().len(), 0);

    // logging a follow-up moves a new visitor to contacted
    let (st, _) = env.call(a, "POST", &format!("/visitors/{id}/follow-ups"), Some(json!({"kind": "call", "note": "Spoke, will come Sunday"}))).await;
    assert_eq!(st, 201);
    assert_eq!(env.call(a, "POST", &format!("/visitors/{id}/follow-ups"), Some(json!({"kind": "carrier pigeon"}))).await.0, 422);
    let (_, g) = env.call(a, "GET", &format!("/visitors/{id}"), None).await;
    assert_eq!((g["data"]["status"].as_str(), g["data"]["follow_up_count"].as_i64()), (Some("contacted"), Some(1)));
    assert_eq!(env.call(a, "GET", &format!("/visitors/{id}/follow-ups"), None).await.1["data"].as_array().unwrap().len(), 1);
    assert_eq!(env.call(a, "GET", "/visitors?status=new", None).await.1["data"].as_array().unwrap().len(), 0);

    // convert to a member
    let (st, c) = env.call(a, "POST", &format!("/visitors/{id}/convert"), Some(json!({"member_id": uid(A1_MEMBER)}))).await;
    assert_eq!(st, 200, "{c}");
    assert_eq!(c["data"]["status"], "joined");
    assert_eq!(env.call(a, "POST", &format!("/visitors/{id}/convert"), Some(json!({"member_id": uid(common::db::B1_ADMIN)}))).await.0, 422, "must be one of our members");

    // members who never checked in show up; attending clears them; a logged contact is remembered
    let (_, ab) = env.call(a, "GET", "/follow-up/absent?weeks=3", None).await;
    assert!(ab["data"].as_array().unwrap().iter().any(|r| r["id"] == uid(A1_MEMBER).to_string()));
    let (st, _) = env.call(a, "POST", &format!("/follow-up/members/{}", uid(A1_MEMBER)), Some(json!({"kind": "message", "note": "Checked in on her"}))).await;
    assert_eq!(st, 201);
    let (_, ab) = env.call(a, "GET", "/follow-up/absent", None).await;
    let row = ab["data"].as_array().unwrap().iter().find(|r| r["id"] == uid(A1_MEMBER).to_string()).unwrap().clone();
    assert_eq!(row["follow_up_count"], 1);

    sqlx::query("INSERT INTO events (title, date, start_time, end_time, location, foc_id, church_id, scope, attendance) VALUES ('Sunday', current_date, '09:00+00', '11:00+00', 'Hall', $1, $2, 'foc', jsonb_build_object($3::text, now()))")
        .bind(uid(FOC_A1)).bind(uid(CHURCH_A)).bind(uid(A1_MEMBER).to_string()).execute(&env.pool).await.unwrap();
    let (_, ab) = env.call(a, "GET", "/follow-up/absent", None).await;
    assert!(!ab["data"].as_array().unwrap().iter().any(|r| r["id"] == uid(A1_MEMBER).to_string()), "recent attender is not absent");

    // the public QR form
    assert_eq!(env.call(m, "GET", "/visitors/form-link", None).await.0, 403);
    let (st, l) = env.call(a, "GET", "/visitors/form-link", None).await;
    assert_eq!(st, 200, "{l}");
    let token = l["data"]["token"].as_str().unwrap().to_string();
    assert!(token.len() >= 60 && l["data"]["enabled"] == true);
    assert_eq!(env.call(a, "GET", "/visitors/form-link", None).await.1["data"]["token"], token.as_str(), "stable");
    let pubp = format!("/public/visit/{token}");
    assert_eq!(env.call_public("GET", &pubp, None).await.1["data"]["church_name"], "A1");
    assert_eq!(env.call_public("GET", "/public/visit/nope", None).await.0, 422);
    assert_eq!(env.call_public("POST", &pubp, Some(json!({"full_name": ""}))).await.0, 422);
    assert_eq!(env.call_public("POST", &pubp, Some(json!({"full_name": "Bot", "website": "http://spam"}))).await.0, 201, "bots look successful");
    assert_eq!(env.call_public("POST", &pubp, Some(json!({"full_name": "Rudo", "phone": "0782", "status": "joined"}))).await.0, 422, "no setting status");
    assert_eq!(env.call_public("POST", &pubp, Some(json!({"full_name": "Rudo", "phone": "0782"}))).await.0, 201);
    assert_eq!(env.call_public("POST", &pubp, Some(json!({"full_name": "Rudo again", "phone": "0782"}))).await.0, 201, "repeat is ignored");
    let (_, all) = env.call(a, "GET", "/visitors", None).await;
    let from_form: Vec<_> = all["data"].as_array().unwrap().iter().filter(|v| v["source"] == "form").collect();
    assert_eq!(from_form.len(), 1);
    assert_eq!((from_form[0]["full_name"].as_str(), from_form[0]["status"].as_str()), (Some("Rudo"), Some("new")));
    // disabling and renewing the link
    env.call(a, "PUT", "/visitors/form-link", Some(json!({"enabled": false}))).await;
    assert_eq!(env.call_public("GET", &pubp, None).await.0, 422);
    let (_, l2) = env.call(a, "PUT", "/visitors/form-link", Some(json!({"enabled": true, "rotate": true}))).await;
    assert_ne!(l2["data"]["token"], token.as_str());
    assert_eq!(env.call_public("GET", &pubp, None).await.0, 422, "old printed code stops working");

    // delete
    assert_eq!(env.call(a, "DELETE", &format!("/visitors/{id}"), None).await.0, 204);
    assert_eq!(env.call(a, "GET", &format!("/visitors/{id}"), None).await.0, 422);
    env.cleanup().await;
}
