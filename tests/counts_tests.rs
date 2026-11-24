//! Hand counts on an event: one fixed list, kept in manual_stats, and what the Attendance report makes of it.
mod common;

use common::db::setup;
use serde_json::json;

#[tokio::test]
async fn counts_are_a_fixed_list_and_feed_the_attendance_report() {
    let Some(env) = setup().await else { return };
    let (a, m) = (&env.a1_admin, &env.a1_member);

    let ev = json!({"title":"Sunday","date":"2026-11-01","start_time":"09:00","end_time":"11:00","location":"Hall","scope":"foc",
                    "manual_stats": {"males": 30, "females": 40, "children": 12, "arena": 8, "visitors": 5, "salvation": 2}});
    let (st, created) = env.call(a, "POST", "/events", Some(ev)).await;
    assert_eq!(st, 201, "{created}");
    let id = created["data"]["id"].as_str().unwrap().to_string();
    assert_eq!(created["data"]["manual_stats"]["arena"], 8);

    // only the counts sent change; negatives are refused; members cannot edit
    let path = format!("/events/{id}");
    let (st, e) = env.call(a, "PATCH", &path, Some(json!({"manual_stats": {"males": 31, "alterCall": 3, "reassurance": 1}}))).await;
    assert_eq!(st, 200, "{e}");
    let s = &e["data"]["manual_stats"];
    assert_eq!((s["males"].as_i64(), s["females"].as_i64(), s["alterCall"].as_i64(), s["reassurance"].as_i64()), (Some(31), Some(40), Some(3), Some(1)));
    assert_eq!(env.call(a, "PATCH", &path, Some(json!({"manual_stats": {"males": -1}}))).await.0, 422);
    assert_eq!(env.call(m, "PATCH", &path, Some(json!({"manual_stats": {"males": 1}}))).await.0, 403);

    // headcount = males + females + children + youth: 31 + 40 + 12 + 8
    let (_, r) = env.call(a, "GET", "/reports/attendance?from=2026-11-01&to=2026-11-30", None).await;
    let svc = r["data"]["services"].as_array().unwrap().iter().find(|s| s["id"] == id.as_str()).expect("service listed");
    assert_eq!((svc["headcount"].as_i64(), svc["visitors"].as_i64(), svc["salvations"].as_i64()), (Some(91), Some(5), Some(2)));
    let counters = r["data"]["counters"].as_array().unwrap();
    let total = |k: &str| counters.iter().find(|c| c["key"] == k).unwrap()["total"].as_i64();
    assert_eq!((total("arena"), total("alterCall"), total("reassurance")), (Some(8), Some(3), Some(1)));
    env.cleanup().await;
}
