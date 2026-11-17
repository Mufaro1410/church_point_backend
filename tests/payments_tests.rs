//! Mobile-money payments through a (fake) Paynow: signing, one income per payment, privacy of the key.
mod common;

use std::sync::{Arc, Mutex};

use axum::{Router, extract::State, routing::post};
use churchpoint_backend::clients::paynow::{parse, sign, verify};
use common::db::{code, setup_custom};
use serde_json::{Value, json};

const KEY: &str = "super-secret-integration-key";

#[derive(Default)]
struct Fake {
    status: String,
    amount: String,
    initiated: Vec<Vec<(String, String)>>,
}
type Shared = Arc<Mutex<Fake>>;

fn reply(pairs: &[(&str, &str)]) -> String {
    let values: Vec<&str> = pairs.iter().map(|(_, v)| *v).collect();
    let hash = sign(&values, KEY);
    let mut s = url::form_urlencoded::Serializer::new(String::new());
    for (k, v) in pairs { s.append_pair(k, v); }
    s.append_pair("hash", &hash);
    s.finish()
}

async fn start_fake() -> (String, Shared) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let shared: Shared = Arc::new(Mutex::new(Fake { status: "Sent".into(), amount: "5.00".into(), ..Default::default() }));
    let b = base.clone();
    let app = Router::new()
        .route("/interface/remotetransaction", post(move |State(f): State<Shared>, body: String| {
            let b = b.clone();
            async move {
                let fields = parse(&body);
                if !verify(&fields, KEY) {
                    return reply(&[("status", "Error"), ("error", "Invalid Integration Key")]);
                }
                f.lock().unwrap().initiated.push(fields);
                reply(&[("status", "Ok"), ("instructions", "Dial *151*2*2#"), ("paynowreference", "PN-1"), ("pollurl", &format!("{b}/interface/poll/1"))])
            }
        }))
        .route("/interface/poll/1", post(|State(f): State<Shared>| async move {
            let (s, a) = { let g = f.lock().unwrap(); (g.status.clone(), g.amount.clone()) };
            reply(&[("reference", "r"), ("paynowreference", "PN-1"), ("amount", &a), ("status", &s), ("pollurl", "x")])
        }))
        .with_state(shared.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, shared)
}

#[tokio::test]
async fn pay_with_ecocash_records_one_income() {
    let (base, fake) = start_fake().await;
    let Some(env) = setup_custom(None, Some(base)).await else { return };
    let a = &env.a1_admin;

    // the church switches on a currency, an income category and a method, then connects Paynow
    env.call(a, "POST", "/finance/currencies", Some(json!({"name":"usd","symbol":"$"}))).await;
    env.call(a, "POST", "/finance/categories", Some(json!({"name":"online giving","type":"income"}))).await;
    let (_, s) = env.call(a, "POST", "/finance/methods", Some(json!({"name":"ecocash"}))).await;
    let id = |list: &str, n: &str| s["data"][list].as_array().unwrap().iter()
        .find(|x| x["name"].as_str().unwrap().eq_ignore_ascii_case(n)).unwrap()["id"].as_str().unwrap().to_string();
    let (usd, cat, method) = (id("currencies", "USD"), id("categories", "Online giving"), id("methods", "Ecocash"));

    let body = json!({"integration_id":"1234","integration_key":KEY,"merchant_email":"church@x.com",
                      "test_mode":true,"category_id":cat,"method_id":method,"enabled":true});
    let (st, saved) = env.call(a, "PUT", "/payments/settings", Some(body)).await;
    assert_eq!(st, 200, "{saved}");
    assert!(!saved.to_string().contains(KEY), "the key must never be sent back");
    let stored: String = sqlx::query_scalar("SELECT integration_key_enc FROM foc_payment_settings").fetch_one(&env.pool).await.unwrap();
    assert!(!stored.contains(KEY), "the key is stored encrypted");

    // members cannot see or change the settings
    let m = &env.a1_member;
    assert_eq!(env.call(m, "GET", "/payments/settings", None).await.0, 403);

    // a bad phone number never reaches Paynow
    let pay = json!({"amount": 5, "currency_id": usd, "method": "ecocash", "phone": "0771234567", "purpose": "Tithe"});
    let (st, b) = env.call(m, "POST", "/payments", Some(json!({"amount":5,"currency_id":usd,"method":"ecocash","phone":"0712345678"}))).await;
    assert_eq!((st.as_u16(), code(&b)), (422, "INVALID_PHONE"));
    assert!(fake.lock().unwrap().initiated.is_empty());

    let (st, p) = env.call(m, "POST", "/payments", Some(pay)).await;
    assert_eq!(st, 201, "{p}");
    assert_eq!(p["data"]["status"], "pending");
    assert_eq!(p["data"]["instructions"], "Dial *151*2*2#");
    let pid = p["data"]["id"].as_str().unwrap().to_string();
    {
        let g = fake.lock().unwrap();
        let sent = &g.initiated[0];
        let get = |k: &str| sent.iter().find(|(n, _)| n == k).unwrap().1.clone();
        assert_eq!((get("amount").as_str(), get("authemail").as_str(), get("method").as_str()), ("5.00", "church@x.com", "ecocash"));
        assert!(get("resulturl").ends_with(&format!("/api/v1/webhooks/paynow/{pid}")));
    }

    // still waiting for the person to approve on their phone
    let (_, g) = env.call(m, "GET", &format!("/payments/{pid}"), None).await;
    assert_eq!(g["data"]["status"], "pending");

    // someone else cannot look at it
    assert_eq!(env.call(&env.a2_admin, "GET", &format!("/payments/{pid}"), None).await.0, 404);

    // a forged webhook is refused
    let forged = "status=Paid&hash=NOPE";
    let (st, _) = env.raw_post(&format!("/webhooks/paynow/{pid}"), forged).await;
    assert_eq!(st, 422);

    // the person approves: asking twice still records exactly one income
    fake.lock().unwrap().status = "Paid".into();
    let (_, g) = env.call(m, "GET", &format!("/payments/{pid}"), None).await;
    assert_eq!(g["data"]["status"], "paid", "{g}");
    assert!(g["data"]["income_id"].is_string());
    env.call(m, "GET", &format!("/payments/{pid}"), None).await;
    let genuine = reply(&[("reference", "r"), ("status", "Paid")]);
    assert_eq!(env.raw_post(&format!("/webhooks/paynow/{pid}"), &genuine).await.0, 200);
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM incomes WHERE description LIKE 'Online payment%'").fetch_one(&env.pool).await.unwrap();
    assert_eq!(n, 1);

    // the church admin sees it in their list
    let (_, list) = env.call(a, "GET", "/payments", None).await;
    assert_eq!(list["data"].as_array().unwrap().len(), 1);
    assert_eq!(list["data"][0]["status"], "paid");
    let _: Value = list;
    env.cleanup().await;
}

#[tokio::test]
async fn payments_need_setup_and_amount_must_match() {
    let (base, fake) = start_fake().await;
    let Some(env) = setup_custom(None, Some(base)).await else { return };
    let a = &env.a1_admin;
    env.call(a, "POST", "/finance/currencies", Some(json!({"name":"usd","symbol":"$"}))).await;
    let (_, s) = env.call(a, "POST", "/finance/categories", Some(json!({"name":"giving","type":"income"}))).await;
    let (_, m) = env.call(a, "POST", "/finance/methods", Some(json!({"name":"ecocash"}))).await;
    let find = |v: &Value, l: &str| v["data"][l].as_array().unwrap().iter().next().unwrap()["id"].as_str().unwrap().to_string();
    let usd = find(&s, "currencies");
    let pay = json!({"amount": 5, "currency_id": usd, "method": "ecocash", "phone": "0771234567"});

    let (st, b) = env.call(&env.a1_member, "POST", "/payments", Some(pay.clone())).await;
    assert_eq!((st.as_u16(), code(&b)), (409, "PAYMENTS_NOT_SET_UP"));

    let body = json!({"integration_id":"1","integration_key":KEY,"merchant_email":"c@x.com","category_id":find(&s,"categories"),
                      "method_id":find(&m,"methods"),"enabled":true});
    assert_eq!(env.call(a, "PUT", "/payments/settings", Some(body)).await.0, 200);
    let (_, p) = env.call(&env.a1_member, "POST", "/payments", Some(pay)).await;
    let pid = p["data"]["id"].as_str().unwrap().to_string();

    // Paynow says "paid" but for a different amount: do not record any income
    { let mut g = fake.lock().unwrap(); g.status = "Paid".into(); g.amount = "1.00".into(); }
    let (_, g) = env.call(&env.a1_member, "GET", &format!("/payments/{pid}"), None).await;
    assert_eq!(g["data"]["status"], "failed");
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM incomes WHERE description LIKE 'Online payment%'").fetch_one(&env.pool).await.unwrap();
    assert_eq!(n, 0);
    env.cleanup().await;
}
