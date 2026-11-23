mod common;

use common::db::{A1_MEMBER, setup, uid};
use serde_json::{Value, json};

#[tokio::test]
async fn overview_and_attendance() {
    let Some(env) = setup().await else { return };
    let (a, m) = (&env.a1_admin, &env.a1_member);

    // Money shows only when asked for by someone allowed; a plain member cannot open reports at all.
    let (st, _) = env.call(m, "GET", "/reports/overview", None).await;
    assert!(st.as_u16() == 403, "member got {st}");

    let (st, o) = env.call(a, "GET", "/reports/overview?from=2026-01-01&to=2026-12-31", None).await;
    assert_eq!(st.as_u16(), 200, "{o}");
    let d = &o["data"];
    assert_eq!(d["period"]["previous_to"], "2025-12-31");
    assert_eq!(d["period"]["previous_from"], "2025-01-01");
    assert!(d["members"]["total"].as_i64().unwrap() >= 1);
    assert!(d["attendance"]["current"]["services"].is_number());
    assert!(d["giving"].is_array());

    // Bad ranges are refused.
    let (st, _) = env.call(a, "GET", "/reports/overview?from=2026-05-01&to=2026-04-01", None).await;
    assert_eq!(st.as_u16(), 422);

    // A service with check-ins shows up in attendance.
    let (_, ev) = env.call(a, "POST", "/events", Some(json!({
        "title": "Report Sunday", "date": "2026-03-01", "start_time": "09:00", "end_time": "11:00",
        "type": "service", "visibility": "church"}))).await;
    if let Some(id) = ev["data"]["id"].as_str() {
        env.call(a, "POST", &format!("/events/{id}/attendance"), Some(json!({"member_id": uid(A1_MEMBER).to_string()}))).await;
    }
    let (st, r) = env.call(a, "GET", "/reports/attendance?from=2026-03-01&to=2026-03-31", None).await;
    assert_eq!(st.as_u16(), 200, "{r}");
    let d: &Value = &r["data"];
    assert!(d["services"].is_array() && d["stopped"].is_array());
    assert!(d["regularity"]["none"].is_number());
}

#[tokio::test]
async fn giving_report() {
    let Some(env) = setup().await else { return };
    let (a, m) = (&env.a1_admin, &env.a1_member);
    let (st, _) = env.call(m, "GET", "/reports/giving", None).await;
    assert_eq!(st.as_u16(), 403);

    env.call(a, "POST", "/finance/currencies", Some(json!({"name":"usd","symbol":"$"}))).await;
    env.call(a, "POST", "/finance/currencies", Some(json!({"name":"zwl","symbol":"Z$","rate":18}))).await;
    env.call(a, "POST", "/finance/categories", Some(json!({"name":"tithes","type":"income"}))).await;
    let (_, s) = env.call(a, "POST", "/finance/methods", Some(json!({"name":"cash"}))).await;
    let id = |l: &str, n: &str| s["data"][l].as_array().unwrap().iter().find(|x| x["name"].as_str().is_some_and(|v| v.eq_ignore_ascii_case(n))).unwrap()["id"].clone();
    let (usd, zwl, cat, cash) = (id("currencies", "USD"), id("currencies", "ZWL"), id("categories", "Tithes"), id("methods", "Cash"));
    let day = chrono::Utc::now().date_naive().to_string();
    for (cur, amt) in [(&usd, 50), (&zwl, 900)] {
        let (st, r) = env.call(a, "POST", "/incomes", Some(json!({"amount": amt, "currency_id": cur, "category_id": cat, "method_id": cash,
            "member_id": uid(A1_MEMBER).to_string(), "date": day}))).await;
        assert!(st.as_u16() < 300, "{r}");
    }
    let (st, r) = env.call(a, "GET", &format!("/reports/giving?from={day}&to={day}"), None).await;
    assert_eq!(st.as_u16(), 200, "{r}");
    let d = &r["data"];
    let tot = d["totals"].as_array().unwrap();
    assert_eq!(tot.len(), 2, "one row per currency: {d}");
    let usd_row = tot.iter().find(|t| t["currency"].as_str().unwrap().eq_ignore_ascii_case("usd")).unwrap();
    assert_eq!(usd_row["current"], "50.00");
    let zwl_row = tot.iter().find(|t| t["currency"].as_str().unwrap().eq_ignore_ascii_case("zwl")).unwrap();
    assert_eq!(zwl_row["usd"], "50.00"); // 900 at 18 per USD
    assert_eq!(d["givers"]["members_giving"], 1);
    assert_eq!(d["by_category"].as_array().unwrap().len(), 2);
    assert!(d["spending"].is_array() && d["pledges"].is_array() && d["monthly"].is_array());
}
