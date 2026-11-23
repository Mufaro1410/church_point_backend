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
async fn pledges_track_giving() {
    let Some(env) = setup().await else { return };
    let ids = enable(&env).await;
    let (a, m, other) = (&env.a1_admin, &env.a1_member, &env.a2_admin);
    let mem = uid(A1_MEMBER).to_string();
    let body = json!({"member_id": mem, "category_id": ids.tithes, "currency_id": ids.usd, "amount": "100", "end_date": "2099-01-01"});

    assert_eq!(env.call(m, "GET", "/pledges", None).await.0, 403);
    assert_eq!(env.call(a, "POST", "/pledges", Some(json!({"member_id": mem, "category_id": ids.rent, "currency_id": ids.usd, "amount": 5}))).await.0, 422, "must be an income category");
    assert_eq!(env.call(a, "POST", "/pledges", Some(json!({"member_id": mem, "category_id": ids.tithes, "currency_id": ids.usd, "amount": 0}))).await.0, 422);
    assert_eq!(env.call(a, "POST", "/pledges", Some(json!({"member_id": uid(25).to_string(), "category_id": ids.tithes, "currency_id": ids.usd, "amount": 5}))).await.0, 422, "other church's member");

    let (st, p) = env.call(a, "POST", "/pledges", Some(body)).await;
    assert_eq!(st, 201, "{p}");
    let id = p["data"]["id"].as_str().unwrap().to_string();
    assert_eq!((p["data"]["paid"].as_str(), p["data"]["fulfilled"].as_bool()), (Some("0.00"), Some(false)));

    // giving by that member in that fund and currency counts; other currency / other member does not
    let mut g = income(&ids, json!("30"), &ids.usd);
    g["member_id"] = json!(mem);
    assert_eq!(env.call(a, "POST", "/incomes", Some(g.clone())).await.0, 201);
    let mut z = income(&ids, json!("999"), &ids.zwl);
    z["member_id"] = json!(mem);
    assert_eq!(env.call(a, "POST", "/incomes", Some(z)).await.0, 201);
    assert_eq!(env.call(a, "POST", "/incomes", Some(income(&ids, json!("500"), &ids.usd))).await.0, 201, "anonymous giving is not theirs");
    let (_, p) = env.call(a, "GET", &format!("/pledges/{id}"), None).await;
    assert_eq!((p["data"]["paid"].as_str(), p["data"]["remaining"].as_str()), (Some("30.00"), Some("70.00")));

    // voided giving stops counting
    let (_, inc) = env.call(a, "GET", "/incomes", None).await;
    let first = inc["data"].as_array().unwrap().iter().find(|i| i["amount"].as_str() == Some("30.00") || i["amount"].as_f64() == Some(30.0)).unwrap()["id"].as_str().unwrap().to_string();
    assert_eq!(env.call(a, "POST", &format!("/incomes/{first}/void"), Some(json!({"reason": "entered twice"}))).await.0, 200);
    let (_, p) = env.call(a, "GET", &format!("/pledges/{id}"), None).await;
    assert_eq!(p["data"]["paid"], "0.00");

    // fulfilled once enough is given; changing and cancelling
    g["amount"] = json!("120");
    env.call(a, "POST", "/incomes", Some(g)).await;
    let (_, p) = env.call(a, "GET", &format!("/pledges/{id}"), None).await;
    assert_eq!((p["data"]["fulfilled"].as_bool(), p["data"]["remaining"].as_str()), (Some(true), Some("0.00")));
    let (st, p) = env.call(a, "PATCH", &format!("/pledges/{id}"), Some(json!({"status": "cancelled", "amount": "150"}))).await;
    assert_eq!((st.as_u16(), p["data"]["status"].as_str()), (200, Some("cancelled")));
    assert_eq!(env.call(a, "PATCH", &format!("/pledges/{id}"), Some(json!({"status": "weird"}))).await.0, 422);
    assert_eq!(env.call(a, "GET", &format!("/pledges?member_id={mem}"), None).await.1["data"].as_array().unwrap().len(), 1);
    assert_eq!(env.call(other, "GET", &format!("/pledges/{id}"), None).await.0, 422, "other church cannot see it");

    // a ministry and an outside organisation can pledge too; giving is attached to the pledge
    let mid = uid(9001);
    sqlx::query("INSERT INTO ministries (id, name) VALUES ($1, 'Youth')").bind(mid).execute(&env.pool).await.unwrap();
    sqlx::query("INSERT INTO foc_ministries (foc_id, ministry_id) VALUES ($1, $2)").bind(uid(11)).bind(mid).execute(&env.pool).await.unwrap();
    let base = |extra: Value| { let mut b = json!({"category_id": ids.tithes, "currency_id": ids.usd, "amount": "200"}); b.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone()); b };
    assert_eq!(env.call(a, "POST", "/pledges", Some(base(json!({})))).await.0, 422, "needs a pledger");
    assert_eq!(env.call(a, "POST", "/pledges", Some(base(json!({"member_id": mem, "pledger_name": "ZB Bank"})))).await.0, 422, "only one pledger");
    assert_eq!(env.call(a, "POST", "/pledges", Some(base(json!({"ministry_id": uid(9999).to_string()})))).await.0, 422, "not our ministry");
    let (st, mp) = env.call(a, "POST", "/pledges", Some(base(json!({"ministry_id": mid.to_string()})))).await;
    assert_eq!(st, 201, "{mp}");
    assert_eq!((mp["data"]["pledger_type"].as_str(), mp["data"]["pledger_name"].as_str()), (Some("ministry"), Some("Youth")));
    let (st, op) = env.call(a, "POST", "/pledges", Some(base(json!({"pledger_name": "ZB Bank"})))).await;
    assert_eq!(st, 201, "{op}");
    assert_eq!((op["data"]["pledger_type"].as_str(), op["data"]["pledger_name"].as_str()), (Some("other"), Some("ZB Bank")));
    let oid = op["data"]["id"].as_str().unwrap().to_string();
    let mut pay = income(&ids, json!("50"), &ids.usd);
    pay["pledge_id"] = json!(oid);
    assert_eq!(env.call(a, "POST", "/incomes", Some(pay.clone())).await.0, 201);
    let (_, op) = env.call(a, "GET", &format!("/pledges/{oid}"), None).await;
    assert_eq!(op["data"]["paid"], "50.00");
    pay["currency_id"] = json!(ids.zwl);
    assert_eq!(env.call(a, "POST", "/incomes", Some(pay)).await.0, 422, "wrong currency for that pledge");
    env.cleanup().await;
}
