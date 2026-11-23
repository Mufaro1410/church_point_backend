mod common;

use common::db::{A1_MEMBER, Env, setup, uid};
use serde_json::{Value, json};

#[allow(dead_code)]
struct Ids { usd: String, zwl: String, tithes: String, rent: String, cash: String }

fn entry(setup: &Value, list: &str, name: &str) -> Value {
    setup["data"][list].as_array().unwrap().iter()
        .find(|x| x["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(name)))
        .unwrap_or_else(|| panic!("{name} not in {list}")).clone()
}
fn pick(setup: &Value, list: &str, name: &str) -> String { entry(setup, list, name)["id"].as_str().unwrap().to_string() }

/// USD (base, rate 1), ZWL at 18 per USD, an income category, an expense category and Cash.
async fn enable(env: &Env) -> Ids {
    let a = &env.a1_admin;
    env.call(a, "POST", "/finance/currencies", Some(json!({"name":"usd","symbol":"$"}))).await;
    env.call(a, "POST", "/finance/currencies", Some(json!({"name":"zwl","symbol":"Z$","rate":18}))).await;
    env.call(a, "POST", "/finance/categories", Some(json!({"name":"tithes","type":"income"}))).await;
    env.call(a, "POST", "/finance/categories", Some(json!({"name":"rent","type":"expense"}))).await;
    let (_, s) = env.call(a, "POST", "/finance/methods", Some(json!({"name":"cash"}))).await;
    Ids {
        usd: pick(&s, "currencies", "USD"), zwl: pick(&s, "currencies", "ZWL"),
        tithes: pick(&s, "categories", "Tithes"), rent: pick(&s, "categories", "Rent"), cash: pick(&s, "methods", "Cash"),
    }
}
fn income(ids: &Ids, amount: Value, currency: &str) -> Value {
    json!({"amount": amount, "currency_id": currency, "category_id": ids.tithes, "method_id": ids.cash})
}



#[tokio::test]
async fn giving_statements() {
    let Some(env) = setup().await else { return };
    let ids = enable(&env).await;
    let (a, m) = (&env.a1_admin, &env.a1_member);
    let mem = uid(A1_MEMBER).to_string();
    let give = |amount: &str, cur: &str, date: &str| { let mut g = income(&ids, json!(amount), cur); g["member_id"] = json!(mem); g["date"] = json!(date); g };
    for g in [give("20", &ids.usd, "2026-02-01"), give("30", &ids.usd, "2026-03-01"), give("500", &ids.zwl, "2026-03-05"), give("99", &ids.usd, "2025-12-31")] {
        assert_eq!(env.call(a, "POST", "/incomes", Some(g)).await.0, 201);
    }
    assert_eq!(env.call(a, "POST", "/incomes", Some(income(&ids, json!("77"), &ids.usd))).await.0, 201, "anonymous giving is on nobody's statement");
    assert_eq!(env.call(m, "GET", "/giving/statements", None).await.0, 403);

    let (_, s) = env.call(a, "GET", "/giving/statements?from=2026-01-01&to=2026-12-31", None).await;
    let row = &s["data"]["members"].as_array().unwrap()[0];
    assert_eq!((row["gifts"].as_i64(), row["totals"]["USD"].as_str(), row["totals"]["ZWL"].as_str()), (Some(3), Some("50.00"), Some("500.00")));

    let (st, d) = env.call(a, "GET", &format!("/giving/statements/{mem}?from=2026-01-01&to=2026-12-31"), None).await;
    assert_eq!(st, 200, "{d}");
    assert_eq!(d["data"]["gifts"].as_array().unwrap().len(), 3);
    assert_eq!(d["data"]["church"]["name"], "A1");
    assert_eq!(d["data"]["by_fund"][0]["fund"], "Tithes");

    // a voided gift drops off; a bad period and other churches' members are refused
    let (_, inc) = env.call(a, "GET", "/incomes", None).await;
    let id = inc["data"].as_array().unwrap().iter().find(|i| i["date"] == "2026-02-01").unwrap()["id"].as_str().unwrap().to_string();
    env.call(a, "POST", &format!("/incomes/{id}/void"), Some(json!({"reason": "entered twice"}))).await;
    let (_, d) = env.call(a, "GET", &format!("/giving/statements/{mem}?from=2026-01-01&to=2026-12-31"), None).await;
    assert_eq!(d["data"]["totals"]["USD"], "30.00");
    assert_eq!(env.call(a, "GET", "/giving/statements?from=2026-05-01&to=2026-01-01", None).await.0, 422);
    assert_eq!(env.call(&env.a2_admin, "GET", &format!("/giving/statements/{mem}"), None).await.0, 422);
    env.cleanup().await;
}
