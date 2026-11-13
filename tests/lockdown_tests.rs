//! "Front door" tests: what someone holding only the public anon key, or an ordinary login, can do
//! straight against the database (bypassing Axum). They run against the replica of YOUR real policies.
mod common;

use common::db::{CHURCH_A, FOC_A1, Env, setup, uid};
use serde_json::Value;

/// One row in each table that used to be wide open, so "0 rows visible" really means "hidden".
async fn put_data_in_formerly_open_tables(env: &Env) {
    let (foc, church) = (uid(FOC_A1), uid(CHURCH_A));
    let (min, cat, cur, meth, cg, ev) = (uid(900), uid(901), uid(902), uid(903), uid(904), uid(905));
    let stmts = [
        format!("insert into ministries(id,name) values ('{min}','Ushers')"),
        format!("insert into foc_ministries(foc_id,ministry_id) values ('{foc}','{min}')"),
        format!("insert into categories(id,name,type) values ('{cat}','Tithes','income')"),
        format!("insert into currencies(id,name,symbol) values ('{cur}','usd','$')"),
        format!("insert into methods(id,name) values ('{meth}','Cash')"),
        format!("insert into foc_categories(foc_id,category_id) values ('{foc}','{cat}')"),
        format!("insert into foc_currencies(foc_id,currency_id) values ('{foc}','{cur}')"),
        format!("insert into foc_methods(foc_id,method_id) values ('{foc}','{meth}')"),
        format!("insert into ministry_leaders(ministry_id,foc_id,leader_id) values ('{min}','{foc}','{}')", uid(21)),
        format!("insert into cell_groups(id,name,foc_id) values ('{cg}','G1','{foc}')"),
        format!("insert into cell_meetings(cell_group_id,date,offering) values ('{cg}',current_date,'[{{\"amount\":5}}]')"),
        format!("insert into events(id,title,date,start_time,end_time,location,scope,foc_id,church_id) values ('{ev}','E',current_date,'09:00','10:00','L','foc','{foc}','{church}')"),
        format!("insert into event_income(event_id,foc_id,currency_id,amount) values ('{ev}','{foc}','{cur}',150)"),
        format!("insert into family_relationships(member_id,related_member_id,relationship_type) values ('{}','{}','spouse')", uid(21), uid(22)),
    ];
    for s in stmts {
        sqlx::query(&s).execute(&env.pool).await.unwrap_or_else(|e| panic!("{s}\n{e}"));
    }
}

const FORMERLY_OPEN: [&str; 8] = [
    "cell_meetings", "event_income", "family_relationships", "foc_categories",
    "foc_currencies", "foc_methods", "foc_ministries", "ministry_leaders",
];

#[tokio::test]
async fn anonymous_visitors_cannot_read_or_write_the_formerly_open_tables() {
    let Some(env) = setup().await else { return };
    put_data_in_formerly_open_tables(&env).await;

    for table in FORMERLY_OPEN {
        assert!(env.count_as("authenticated", Some(uid(121)), table).await >= 1, "{table}: logged-in users keep access");
        assert_eq!(env.count_as("anon", None, table).await, 0, "{table}: anon must see nothing");
    }
    // writes are refused too
    let r = env.try_as("anon", None, &format!("insert into event_income(event_id,foc_id,currency_id,amount) values ('{}','{}','{}',1)", uid(905), uid(FOC_A1), uid(902))).await;
    assert!(r.is_err(), "anon insert should be refused: {r:?}");
    let r = env.try_as("anon", None, "delete from cell_meetings").await;
    assert_eq!(r, Ok(0), "anon delete must touch nothing");

    // province stays readable before login on purpose; changing it needs a login
    assert!(env.count_as("anon", None, "province").await >= 1);
    assert!(env.try_as("anon", None, "insert into province(name,church_id) values ('X', (select id from church limit 1))").await.is_err());
    env.cleanup().await;
}

#[tokio::test]
async fn members_cannot_escalate_their_own_privileges() {
    let Some(env) = setup().await else { return };
    let member_user = Some(uid(123)); // a1_member's auth user
    let member = uid(23);

    for (what, sql) in [
        ("make self admin", format!("update member_profiles set role='admin' where id='{member}'")),
        ("hijack user_id", format!("update member_profiles set user_id='{}' where id='{member}'", uid(121))),
    ] {
        let r = env.try_as("authenticated", member_user, &sql).await;
        assert!(r.as_ref().is_err_and(|e| e.contains("cannot change role or user_id")), "{what}: {r:?}");
    }
    // editing their own ordinary details still works
    assert_eq!(env.try_as("authenticated", member_user, &format!("update member_profiles set first_name='Mo' where id='{member}'")).await, Ok(1));

    // self-approval
    sqlx::query("insert into auth.users(id,email) values ($1,'p@x.com')").bind(uid(777)).execute(&env.pool).await.unwrap();
    sqlx::query("insert into member_profiles(id,user_id,foc_id,first_name,last_name,gender,phone,role,status) values ($1,$1,$2,'P','Q','F','1','member','pending')")
        .bind(uid(777)).bind(uid(FOC_A1)).execute(&env.pool).await.unwrap();
    let r = env.try_as("authenticated", Some(uid(777)), &format!("update member_profiles set status='approved' where id='{}'", uid(777))).await;
    assert!(r.as_ref().is_err_and(|e| e.contains("cannot approve")), "{r:?}");

    // a brand-new user may only create a pending member profile for themselves
    sqlx::query("insert into auth.users(id,email) values ($1,'n@x.com')").bind(uid(778)).execute(&env.pool).await.unwrap();
    let ins = |role: &str, status: &str| format!("insert into member_profiles(user_id,first_name,last_name,gender,phone,role,status) values ('{}','N','M','F','2','{role}','{status}')", uid(778));
    assert!(env.try_as("authenticated", Some(uid(778)), &ins("admin", "approved")).await.is_err());
    assert!(env.try_as("authenticated", Some(uid(778)), &ins("member", "approved")).await.is_err());
    assert_eq!(env.try_as("authenticated", Some(uid(778)), &ins("member", "pending")).await, Ok(1));

    // legitimate paths keep working: a real admin, the Axum connection, and the signup function
    assert_eq!(env.try_as("authenticated", Some(uid(121)), &format!("update member_profiles set status='approved' where id='{}'", uid(777))).await, Ok(1), "admin approves");
    assert_eq!(env.try_as("postgres", None, &format!("update member_profiles set role='admin' where id='{member}'")).await, Ok(1), "server connection");

    sqlx::query("insert into auth.users(id,email) values ($1,'f@x.com')").bind(uid(779)).execute(&env.pool).await.unwrap();
    let res: Value = env.json_as("authenticated", Some(uid(779)), &format!(
        "select public.create_foc_and_user(
           '{{\"name\":\"New FOC\",\"address\":\"a\",\"email\":\"n@f.com\",\"contact\":\"1\",\"church_id\":\"{}\"}}'::jsonb,
           '{{\"first_name\":\"F\",\"last_name\":\"O\",\"email\":\"f@x.com\",\"phone\":\"3\",\"gender\":\"M\"}}'::jsonb)", uid(CHURCH_A))).await.unwrap();
    assert_eq!(res["success"], true, "signup function (SECURITY DEFINER) must still create the admin: {res}");
    env.cleanup().await;
}

/// TRIPWIRE for the exposure that still exists today (closing it needs the web and mobile apps to stop
/// reading these tables directly, which is Phase 5-6). When that lands, flip these assertions.
#[tokio::test]
async fn known_open_anon_can_still_read_members_and_payment_details() {
    let Some(env) = setup().await else { return };
    sqlx::query(r#"update foc_profile set payment_details = '{"bank_accounts":[{"account_number":"000123456987"}]}' where id = $1"#)
        .bind(uid(FOC_A1)).execute(&env.pool).await.unwrap();

    assert!(env.count_as("anon", None, "member_profiles").await >= 1, "anon can read member phone numbers today");
    let leaked = env.json_as("anon", None, "select to_jsonb(array_agg(payment_details)) from foc_profile").await.unwrap();
    assert!(leaked.to_string().contains("000123456987"), "anon can read bank account numbers today");
    env.cleanup().await;
}
