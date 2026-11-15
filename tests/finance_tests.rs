mod common;

use axum::http::StatusCode;
use common::db::{Env, FOC_A1, code, raw_baseline, seed, setup, uid};
use serde_json::{Value, json};

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
fn expense(ids: &Ids, amount: Value) -> Value {
    json!({"amount": amount, "currency_id": ids.usd, "category_id": ids.rent, "method_id": ids.cash, "supplier": "ZESA"})
}

#[tokio::test]
async fn catalogue_setup_and_rates() {
    let Some(env) = setup().await else { return };
    let ids = enable(&env).await;
    let a = &env.a1_admin;
    let (_, s) = env.call(a, "GET", "/finance/setup", None).await;
    assert_eq!(entry(&s, "currencies", "USD")["rate"], "1.000000");
    assert_eq!(entry(&s, "currencies", "ZWL")["rate"], "18.000000");
    assert_eq!(entry(&s, "categories", "Tithes")["type"], "income");

    let (st, _) = env.call(a, "PUT", &format!("/finance/currencies/{}", ids.usd), Some(json!({"selected":true,"rate":5}))).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "USD is always 1");
    let (st, _) = env.call(a, "POST", "/finance/currencies", Some(json!({"name":"US"}))).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let (st, b) = env.call(a, "POST", "/finance/categories", Some(json!({"name":"Tithes","type":"expense"}))).await;
    assert_eq!((st, code(&b)), (StatusCode::CONFLICT, "CATEGORY_TYPE_CONFLICT"));

    let (_, s) = env.call(a, "PUT", &format!("/finance/currencies/{}", ids.zwl), Some(json!({"selected":true,"rate":"20.5"}))).await;
    assert_eq!(entry(&s, "currencies", "ZWL")["rate"], "20.500000");
    let (_, s) = env.call(a, "PUT", &format!("/finance/currencies/{}", ids.zwl), Some(json!({"selected":false}))).await;
    assert_eq!(entry(&s, "currencies", "ZWL")["selected"], false);
    env.cleanup().await;
}

#[tokio::test]
async fn amounts_convert_to_usd_and_the_rate_is_snapshotted() {
    let Some(env) = setup().await else { return };
    let ids = enable(&env).await;
    let a = &env.a1_admin;

    let (st, first) = env.call(a, "POST", "/incomes", Some(income(&ids, json!(180), &ids.zwl))).await;
    assert_eq!(st, StatusCode::CREATED);
    assert_eq!((first["data"]["amount"].as_str(), first["data"]["amount_usd"].as_str(), first["data"]["exchange_rate"].as_str()),
               (Some("180.00"), Some("10.00"), Some("18.000000")));
    assert_eq!(first["data"]["recorded_by"], env.a1_admin.member_id.to_string());
    assert_eq!(first["data"]["currency_symbol"], "Z$");

    // the FOC changes the rate: history must NOT change
    env.call(a, "PUT", &format!("/finance/currencies/{}", ids.zwl), Some(json!({"selected":true,"rate":20}))).await;
    let id = first["data"]["id"].as_str().unwrap();
    let (_, again) = env.call(a, "GET", &format!("/incomes/{id}"), None).await;
    assert_eq!(again["data"]["amount_usd"], "10.00");
    let (_, second) = env.call(a, "POST", "/incomes", Some(income(&ids, json!(200), &ids.zwl))).await;
    assert_eq!((second["data"]["amount_usd"].as_str(), second["data"]["exchange_rate"].as_str()), (Some("10.00"), Some("20.000000")));

    // USD, as a string and as a number
    for amount in [json!("25.50"), json!(25.5)] {
        let (_, r) = env.call(a, "POST", "/incomes", Some(income(&ids, amount, &ids.usd))).await;
        assert_eq!((r["data"]["amount"].as_str(), r["data"]["amount_usd"].as_str()), (Some("25.50"), Some("25.50")));
    }
    env.cleanup().await;
}

#[tokio::test]
async fn invalid_records_are_refused_with_clear_codes() {
    let Some(env) = setup().await else { return };
    let ids = enable(&env).await;
    let a = &env.a1_admin;
    let post = |body: Value| async { env.call(a, "POST", "/incomes", Some(body)).await };

    for bad in [json!(0), json!(-5), json!("1.005"), json!("1000000000000"), json!("abc")] {
        let (st, _) = post(income(&ids, bad.clone(), &ids.usd)).await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "amount {bad}");
    }
    let mut wrong = income(&ids, json!(5), &ids.usd);
    wrong["category_id"] = json!(ids.rent);
    assert_eq!(code(&post(wrong).await.1), "WRONG_CATEGORY_TYPE");

    // a currency that exists but has no rate for this FOC
    let (_, s) = env.call(a, "POST", "/finance/currencies", Some(json!({"name":"zar","symbol":"R"}))).await;
    assert_eq!(code(&post(income(&ids, json!(5), &pick(&s, "currencies", "ZAR"))).await.1), "MISSING_RATE");

    // things this FOC has switched off
    env.call(a, "PUT", &format!("/finance/currencies/{}", ids.zwl), Some(json!({"selected":false}))).await;
    assert_eq!(code(&post(income(&ids, json!(5), &ids.zwl)).await.1), "CURRENCY_NOT_ENABLED");
    env.call(a, "PUT", &format!("/finance/methods/{}", ids.cash), Some(json!({"selected":false}))).await;
    assert_eq!(code(&post(income(&ids, json!(5), &ids.usd)).await.1), "METHOD_NOT_ENABLED");
    env.call(a, "PUT", &format!("/finance/methods/{}", ids.cash), Some(json!({"selected":true}))).await;
    env.call(a, "PUT", &format!("/finance/categories/{}", ids.tithes), Some(json!({"selected":false}))).await;
    assert_eq!(code(&post(income(&ids, json!(5), &ids.usd)).await.1), "CATEGORY_NOT_ENABLED");
    env.call(a, "PUT", &format!("/finance/categories/{}", ids.tithes), Some(json!({"selected":true}))).await;

    // ownership of the things a record points at
    let mut other_member = income(&ids, json!(5), &ids.usd);
    other_member["member_id"] = json!(uid(25)); // a member of another church
    assert_eq!(post(other_member).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let (_, foreign_event) = env.call(&env.b1_admin, "POST", "/events", Some(json!(
        {"title":"B","date":"2026-12-01","start_time":"09:00","end_time":"10:00","location":"L","scope":"foc"}))).await;
    let mut other_event = income(&ids, json!(5), &ids.usd);
    other_event["event_id"] = foreign_event["data"]["id"].clone();
    assert_eq!(post(other_event).await.0, StatusCode::UNPROCESSABLE_ENTITY);

    let (_, own_event) = env.call(a, "POST", "/events", Some(json!(
        {"title":"A","date":"2026-12-01","start_time":"09:00","end_time":"10:00","location":"L","scope":"foc"}))).await;
    let mut ok = income(&ids, json!(5), &ids.usd);
    ok["event_id"] = own_event["data"]["id"].clone();
    ok["member_id"] = json!(env.a1_member.member_id);
    let (st, r) = post(ok).await;
    assert_eq!((st, r["data"]["event_id"].clone()), (StatusCode::CREATED, own_event["data"]["id"].clone()));
    assert_eq!(r["data"]["member_name"], "Mary Member");

    // each side keeps to its own fields
    let mut with_supplier = income(&ids, json!(5), &ids.usd);
    with_supplier["supplier"] = json!("x");
    assert_eq!(post(with_supplier).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let mut no_supplier = expense(&ids, json!(5));
    no_supplier.as_object_mut().unwrap().remove("supplier");
    assert_eq!(env.call(a, "POST", "/expenses", Some(no_supplier)).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    env.cleanup().await;
}

#[tokio::test]
async fn changes_are_audited_and_records_are_voided_never_deleted() {
    let Some(env) = setup().await else { return };
    let ids = enable(&env).await;
    let a = &env.a1_admin;
    let (_, created) = env.call(a, "POST", "/incomes", Some(income(&ids, json!(10), &ids.usd))).await;
    let id = created["data"]["id"].as_str().unwrap().to_string();

    let (st, b) = env.call(a, "PATCH", &format!("/incomes/{id}"), Some(json!({"amount": 30}))).await;
    assert_eq!((st, code(&b)), (StatusCode::UNPROCESSABLE_ENTITY, "VALIDATION_ERROR"), "a reason is required");
    let (st, u) = env.call(a, "PATCH", &format!("/incomes/{id}"), Some(json!({"reason":"typo","amount":"30"}))).await;
    assert_eq!((st, u["data"]["amount"].as_str()), (StatusCode::OK, Some("30.00")));
    let (_, u) = env.call(a, "PATCH", &format!("/incomes/{id}"), Some(json!({"reason":"paid in ZWL","currency_id": ids.zwl}))).await;
    assert_eq!(u["data"]["exchange_rate"], "18.000000", "a changed currency re-snapshots the rate");

    let (_, h) = env.call(a, "GET", &format!("/finance/history/{id}"), None).await;
    let h = h["data"].as_array().unwrap();
    assert_eq!(h.iter().map(|r| r["action"].as_str().unwrap()).collect::<Vec<_>>(), ["update", "update", "create"]);
    assert_eq!(h[1]["before_data"]["amount"], "10.00");
    assert_eq!(h[1]["after_data"]["amount"], "30.00");
    assert_eq!(h[1]["reason"], "typo");

    let (st, v) = env.call(a, "POST", &format!("/incomes/{id}/void"), Some(json!({"reason":"duplicate entry"}))).await;
    assert_eq!(st, StatusCode::OK);
    assert!(v["data"]["voided_at"].is_string());
    let (_, l) = env.call(a, "GET", "/incomes", None).await;
    assert_eq!(l["data"].as_array().unwrap().len(), 0, "voided records leave the normal list");
    let (_, l) = env.call(a, "GET", "/incomes?include_voided=true", None).await;
    assert_eq!(l["data"].as_array().unwrap().len(), 1);
    let (st, b) = env.call(a, "PATCH", &format!("/incomes/{id}"), Some(json!({"reason":"again","amount":1}))).await;
    assert_eq!((st, code(&b)), (StatusCode::CONFLICT, "ENTRY_VOIDED"));
    let (st, _) = env.call(a, "POST", &format!("/incomes/{id}/void"), Some(json!({"reason":"twice"}))).await;
    assert_eq!(st, StatusCode::CONFLICT);
    let (st, _) = env.call(a, "DELETE", &format!("/incomes/{id}"), None).await;
    assert_eq!(st, StatusCode::METHOD_NOT_ALLOWED, "there is no delete");

    let rows: i64 = sqlx::query_scalar("select count(*) from incomes").fetch_one(&env.pool).await.unwrap();
    assert_eq!(rows, 1);
    env.cleanup().await;
}

#[tokio::test]
async fn the_summary_adds_up_in_usd_and_ignores_voided_records() {
    let Some(env) = setup().await else { return };
    let ids = enable(&env).await;
    let a = &env.a1_admin;
    env.call(a, "POST", "/incomes", Some(income(&ids, json!(100), &ids.usd))).await;
    env.call(a, "POST", "/incomes", Some(income(&ids, json!(180), &ids.zwl))).await; // 10 USD at 18
    let (_, extra) = env.call(a, "POST", "/incomes", Some(income(&ids, json!(50), &ids.usd))).await;
    env.call(a, "POST", &format!("/incomes/{}/void", extra["data"]["id"].as_str().unwrap()), Some(json!({"reason":"entered twice"}))).await;
    env.call(a, "POST", "/expenses", Some(expense(&ids, json!(40)))).await;

    let (_, s) = env.call(a, "GET", "/finance/summary", None).await;
    let d = &s["data"];
    assert_eq!(d["income"]["total_usd"], "110.00");
    assert_eq!(d["income"]["count"], 2);
    assert_eq!(d["expense"]["total_usd"], "40.00");
    assert_eq!(d["net_usd"], "70.00");
    assert_eq!(d["income"]["by_currency"].as_array().unwrap().len(), 2);
    assert_eq!(d["income"]["by_category"][0]["category"], "Tithes");
    assert_eq!(d["income"]["by_method"][0]["method"], "Cash");

    let (_, empty) = env.call(a, "GET", "/finance/summary?start=2001-01-01&end=2001-12-31", None).await;
    assert_eq!(empty["data"]["income"]["count"], 0);
    env.cleanup().await;
}

#[tokio::test]
async fn other_focs_and_ordinary_members_cannot_reach_the_ledger() {
    let Some(env) = setup().await else { return };
    let ids = enable(&env).await;
    let (_, r) = env.call(&env.a1_admin, "POST", "/incomes", Some(income(&ids, json!(10), &ids.usd))).await;
    let id = r["data"]["id"].as_str().unwrap();

    let b = &env.b1_admin;
    for (m, uri, body) in [
        ("GET", format!("/incomes/{id}"), None),
        ("PATCH", format!("/incomes/{id}"), Some(json!({"reason":"hijack","amount":1}))),
        ("POST", format!("/incomes/{id}/void"), Some(json!({"reason":"hijack"}))),
    ] {
        let (st, e) = env.call(b, m, &uri, body).await;
        assert_eq!((st, code(&e)), (StatusCode::NOT_FOUND, "ENTRY_NOT_FOUND"), "{m} {uri}");
    }
    let (_, l) = env.call(b, "GET", "/incomes", None).await;
    assert_eq!(l["data"].as_array().unwrap().len(), 0);
    let (_, h) = env.call(b, "GET", &format!("/finance/history/{id}"), None).await;
    assert_eq!(h["data"].as_array().unwrap().len(), 0);

    for uri in ["/incomes", "/expenses", "/finance/setup", "/finance/summary"] {
        assert_eq!(env.call(&env.a1_member, "GET", uri, None).await.0, StatusCode::FORBIDDEN, "{uri}");
    }
    env.cleanup().await;
}

#[tokio::test]
async fn migration_converts_float_amounts_and_merges_event_income() {
    let Some(db) = raw_baseline().await else { return };
    db.migrate_through(20261112090200).await; // everything BEFORE the finance migration
    seed(&db.su).await;
    let (foc, usd, tithes, event_cat, cash, ev) = (uid(FOC_A1), uid(900), uid(901), uid(902), uid(903), uid(904));
    for s in [
        format!("insert into currencies(id,name,symbol,rate) values ('{usd}','usd','$',1)"),
        format!("insert into categories(id,name,type) values ('{tithes}','Tithes','income'),('{event_cat}','Event Income','income')"),
        format!("insert into methods(id,name) values ('{cash}','Cash')"),
        format!("insert into foc_currencies(foc_id,currency_id) values ('{foc}','{usd}')"),
        format!("insert into events(id,title,date,start_time,end_time,location,scope,foc_id,church_id) values ('{ev}','E',current_date,'09:00','10:00','L','foc','{foc}',(select church_id from foc_profile where id='{foc}'))"),
        format!("insert into event_income(event_id,foc_id,category_id,currency_id,amount,method,notes) values ('{ev}','{foc}',null,'{usd}',150,'cash','Sunday'),('{ev}','{foc}','{tithes}','{usd}',0,'cash','')"),
        format!("insert into incomes(foc_id,category_id,amount,currency_id,method_id,date) values ('{foc}','{tithes}',10.1,'{usd}','{cash}',current_date)"),
    ] {
        sqlx::query(&s).execute(&db.su).await.unwrap_or_else(|e| panic!("{s}\n{e}"));
    }

    db.migrate_through(i64::MAX).await;

    let rows: Vec<(String, Option<String>, Option<uuid::Uuid>, bool, uuid::Uuid, uuid::Uuid)> = sqlx::query_as(
        "select amount::text, exchange_rate::text, event_id, legacy_event_income_id is not null, category_id, method_id from incomes order by amount")
        .fetch_all(&db.su).await.unwrap();
    assert_eq!(rows, vec![
        ("10.10".to_string(), Some("1.000000".to_string()), None, false, tithes, cash),
        ("150.00".to_string(), Some("1.000000".to_string()), Some(ev), true, event_cat, cash),
    ]);
    db.cleanup().await;
}
