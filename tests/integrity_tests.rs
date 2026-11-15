//! Phase 2: deletes must not destroy data, identity must be unambiguous, testimonies use member ids.
mod common;

use axum::http::StatusCode;
use common::db::{FOC_A1, code, raw_baseline, seed, setup, uid};
use serde_json::json;

/// A currency, category, method, ministry and one income + one expense that use them.
async fn put_finance_data(pool: &sqlx::PgPool) {
    let (foc, member) = (uid(FOC_A1), uid(23));
    for s in [
        format!("insert into ministries(id,name) values ('{}','Ushers')", uid(900)),
        format!("insert into categories(id,name,type) values ('{}','Tithes','income'), ('{}','Rent','expense')", uid(901), uid(906)),
        format!("insert into currencies(id,name,symbol) values ('{}','usd','$')", uid(902)),
        format!("insert into methods(id,name) values ('{}','Cash')", uid(903)),
        format!("insert into incomes(member_id,foc_id,category_id,amount,currency_id,method_id) values ('{member}','{foc}','{}',10,'{}','{}')", uid(901), uid(902), uid(903)),
        format!("insert into expenses(supplier,foc_id,ministry_id,amount,currency_id,method_id,category_id) values ('ZESA','{foc}','{}',5,'{}','{}','{}')", uid(900), uid(902), uid(903), uid(906)),
    ] {
        sqlx::query(&s).execute(pool).await.unwrap_or_else(|e| panic!("{s}\n{e}"));
    }
}

#[tokio::test]
async fn deleting_a_cell_group_keeps_its_members() {
    let Some(env) = setup().await else { return };
    sqlx::query("insert into cell_groups(id,name,foc_id) values ($1,'G1',$2)").bind(uid(950)).bind(uid(FOC_A1)).execute(&env.pool).await.unwrap();
    sqlx::query("update member_profiles set cell_group_id = $1 where id = $2").bind(uid(950)).bind(uid(23)).execute(&env.pool).await.unwrap();

    sqlx::query("delete from cell_groups where id = $1").bind(uid(950)).execute(&env.pool).await.unwrap();

    let (exists, group): (bool, Option<uuid::Uuid>) =
        sqlx::query_as("select true, cell_group_id from member_profiles where id = $1").bind(uid(23)).fetch_one(&env.pool).await.unwrap();
    assert!(exists && group.is_none(), "the member must survive, just without a group");
    env.cleanup().await;
}

#[tokio::test]
async fn money_and_the_things_it_points_to_cannot_be_deleted_by_accident() {
    let Some(env) = setup().await else { return };
    put_finance_data(&env.pool).await;
    sqlx::query("insert into events(id,title,date,start_time,end_time,location,scope,foc_id,church_id) values ($1,'E',current_date,'09:00','10:00','L','foc',$2,(select church_id from foc_profile where id=$2))")
        .bind(uid(960)).bind(uid(FOC_A1)).execute(&env.pool).await.unwrap();
    sqlx::query("insert into event_income(event_id,foc_id,currency_id,amount) values ($1,$2,$3,150)")
        .bind(uid(960)).bind(uid(FOC_A1)).bind(uid(902)).execute(&env.pool).await.unwrap();

    for (what, sql) in [
        ("currency", format!("delete from currencies where id='{}'", uid(902))),
        ("income category", format!("delete from categories where id='{}'", uid(901))),
        ("expense category", format!("delete from categories where id='{}'", uid(906))),
        ("payment method", format!("delete from methods where id='{}'", uid(903))),
        ("ministry with expenses", format!("delete from ministries where id='{}'", uid(900))),
        ("event with income", format!("delete from events where id='{}'", uid(960))),
        ("FOC with members and money", format!("delete from foc_profile where id='{}'", uid(FOC_A1))),
        ("member who recorded income", format!("delete from member_profiles where id='{}'", uid(23))),
    ] {
        let r = sqlx::query(&sql).execute(&env.pool).await;
        assert!(r.as_ref().is_err_and(|e| e.to_string().contains("foreign key")), "{what} must be protected: {r:?}");
    }
    let (inc, exp): (i64, i64) = sqlx::query_as("select (select count(*) from incomes), (select count(*) from expenses)").fetch_one(&env.pool).await.unwrap();
    assert_eq!((inc, exp), (1, 1));
    env.cleanup().await;
}

#[tokio::test]
async fn the_api_refuses_to_delete_an_event_that_has_income() {
    let Some(env) = setup().await else { return };
    put_finance_data(&env.pool).await;
    let (s, ev) = env.call(&env.a1_admin, "POST", "/events", Some(json!(
        {"title":"Gala","date":"2026-12-12","start_time":"09:00","end_time":"12:00","location":"Hall","scope":"foc"}))).await;
    assert_eq!(s, StatusCode::CREATED);
    let id = ev["data"]["id"].as_str().unwrap();
    sqlx::query("insert into event_income(event_id,foc_id,currency_id,amount) values ($1::uuid,$2,$3,150)")
        .bind(id).bind(uid(FOC_A1)).bind(uid(902)).execute(&env.pool).await.unwrap();

    let (s, b) = env.call(&env.a1_admin, "DELETE", &format!("/events/{id}"), None).await;
    assert_eq!((s, code(&b)), (StatusCode::CONFLICT, "EVENT_IN_USE"));
    let (s, _) = env.call(&env.a1_admin, "GET", &format!("/events/{id}"), None).await;
    assert_eq!(s, StatusCode::OK, "the event is still there");
    env.cleanup().await;
}

#[tokio::test]
async fn a_login_and_an_email_belong_to_one_profile() {
    let Some(env) = setup().await else { return };
    let new_profile = |user: uuid::Uuid, email: &str, role: &str| {
        let email = email.to_string();
        let role = role.to_string();
        let pool = env.pool.clone();
        async move {
            sqlx::query("insert into auth.users(id,email) values ($1,$2) on conflict do nothing").bind(user).bind(&email).execute(&pool).await.unwrap();
            sqlx::query("insert into member_profiles(user_id,foc_id,email,first_name,last_name,gender,phone,role) values ($1,$2,$3,'N','M','F','1',$4)")
                .bind(user).bind(uid(FOC_A1)).bind(&email).bind(&role).execute(&pool).await
        }
    };
    // a second profile for an existing login
    let r = sqlx::query("insert into member_profiles(user_id,foc_id,first_name,last_name,gender,phone) values ($1,$2,'D','D','F','1')")
        .bind(uid(121)).bind(uid(FOC_A1)).execute(&env.pool).await;
    assert!(r.is_err_and(|e| e.to_string().contains("member_profiles_user_id_key")));
    // same email in a different case
    let r = new_profile(uid(800), "ADA@X.COM", "member").await;
    assert!(r.is_err_and(|e| e.to_string().contains("member_profiles_email_unique_ci")));
    // ...but a soft-deleted profile frees its email
    sqlx::query("update member_profiles set deleted_at = now() where id = $1").bind(uid(21)).execute(&env.pool).await.unwrap();
    assert!(new_profile(uid(801), "ADA@X.COM", "member").await.is_ok());
    // roles are a closed list
    assert!(new_profile(uid(802), "w@x.com", "wizard").await.is_err_and(|e| e.to_string().contains("member_profiles_role_check")));
    assert!(new_profile(uid(803), "s@x.com", "super_admin").await.is_ok());
    env.cleanup().await;
}

#[tokio::test]
async fn the_busy_tables_are_indexed() {
    let Some(env) = setup().await else { return };
    let have: Vec<String> = sqlx::query_scalar("select indexname::text from pg_indexes where schemaname='public'").fetch_all(&env.pool).await.unwrap();
    for ix in ["events_foc_date_idx", "events_church_date_idx", "incomes_foc_date_idx", "expenses_foc_date_idx",
               "prayers_foc_created_idx", "sermons_foc_date_idx", "incomes_currency_idx", "expenses_ministry_idx"] {
        assert!(have.iter().any(|i| i == ix), "missing index {ix}");
    }
    env.cleanup().await;
}

#[tokio::test]
async fn dead_otp_functions_are_gone_and_the_category_lock_is_fixed() {
    let Some(env) = setup().await else { return };
    let left: i64 = sqlx::query_scalar("select count(*) from pg_proc where pronamespace='public'::regnamespace and proname in ('send_whatsapp_otp','verify_whatsapp_otp','cleanup_expired_otps','fn_member_profile_exists')")
        .fetch_one(&env.pool).await.unwrap();
    assert_eq!(left, 0);

    put_finance_data(&env.pool).await;
    // a category used only by an EXPENSE: a no-op update used to be refused (operator-precedence bug)
    let same = sqlx::query("update categories set type = 'expense' where id = $1").bind(uid(906)).execute(&env.pool).await;
    assert!(same.is_ok(), "no-op type update must be allowed: {same:?}");
    // a real change on a category that is in use is still refused
    let change = sqlx::query("update categories set type = 'income' where id = $1").bind(uid(906)).execute(&env.pool).await;
    assert!(change.is_err_and(|e| e.to_string().contains("Cannot change type")));
    let change = sqlx::query("update categories set type = 'expense' where id = $1").bind(uid(901)).execute(&env.pool).await;
    assert!(change.is_err_and(|e| e.to_string().contains("Cannot change type")));
    env.cleanup().await;
}

// ───────────────────────── testimonies: auth ids -> member ids ─────────────────────────

#[tokio::test]
async fn migration_converts_old_testimony_owners_to_member_ids() {
    let Some(db) = raw_baseline().await else { return };
    db.migrate_through(20261112090100).await; // everything BEFORE the testimonies migration
    seed(&db.su).await;

    let insert = "insert into testimonies(foc_id,profile_id,title,situation,intervention,outcome,author) values ($1,$2,$3,'s','i','o','Someone')";
    let orphan = uid(999); // an auth id that matches no member
    for (profile, title) in [(Some(uid(121)), "by auth id"), (Some(orphan), "orphan"), (None, "no owner")] {
        sqlx::query(insert).bind(uid(FOC_A1)).bind(profile).bind(title).execute(&db.su).await.unwrap();
    }

    db.migrate_through(i64::MAX).await;

    let rows: Vec<(String, Option<uuid::Uuid>)> = sqlx::query_as("select title, profile_id from testimonies order by title").fetch_all(&db.su).await.unwrap();
    assert_eq!(rows, vec![
        ("by auth id".to_string(), Some(uid(21))), // auth user 121 -> member 21
        ("no owner".to_string(), None),
        ("orphan".to_string(), None),
    ]);
    // the new foreign key refuses a made-up owner
    let r = sqlx::query(insert).bind(uid(FOC_A1)).bind(uid(998)).bind("forged").execute(&db.su).await;
    assert!(r.is_err_and(|e| e.to_string().contains("testimonies_profile_id_fkey")));
    db.cleanup().await;
}

#[tokio::test]
async fn testimony_row_policies_follow_the_member_id() {
    let Some(env) = setup().await else { return };
    let (member_user, other_user, outsider) = (Some(uid(123)), Some(uid(122)), Some(uid(124)));
    let insert = |owner: u32| format!("insert into testimonies(foc_id,profile_id,title,situation,intervention,outcome,author) values ('{}','{}','Mine','s','i','o','Mary')", uid(FOC_A1), uid(owner));

    assert_eq!(env.try_as("authenticated", member_user, &insert(23)).await, Ok(1), "own member id");
    assert!(env.try_as("authenticated", member_user, &insert(21)).await.is_err(), "someone else's id is refused");
    assert!(env.try_as("anon", None, &insert(23)).await.is_err());

    sqlx::query(&insert(23)).execute(&env.pool).await.unwrap(); // a pending testimony owned by member 23
    assert_eq!(env.count_as("authenticated", member_user, "testimonies").await, 1, "the author sees their own pending testimony");
    assert_eq!(env.count_as("authenticated", other_user, "testimonies").await, 0, "a colleague does not");
    assert_eq!(env.count_as("authenticated", outsider, "testimonies").await, 0, "nor another church");

    let upd = "update testimonies set title = 'Edited'";
    assert_eq!(env.try_as("authenticated", member_user, upd).await, Ok(1), "own pending testimony is editable");
    sqlx::query("update testimonies set status = 'approved'").execute(&env.pool).await.unwrap();
    assert_eq!(env.try_as("authenticated", member_user, upd).await, Ok(0), "approved testimonies are locked");
    assert_eq!(env.count_as("authenticated", other_user, "testimonies").await, 1, "approved ones are visible to the FOC");
    env.cleanup().await;
}
