//! Tests of the schema and the migrations themselves.
mod common;

use common::db::{MIGRATOR, raw_baseline, setup};
use serde_json::Value;
use std::collections::BTreeMap;

const DIGEST: &str = r#"select json_object_agg(table_name, cols) from (
  select table_name, string_agg(column_name || ' ' || udt_name
      || case when is_nullable = 'NO' then ' NOT NULL' else '' end
      || coalesce(' DEFAULT ' || column_default, ''), chr(10) order by ordinal_position) as cols
  from information_schema.columns where table_schema = 'public' group by table_name) t"#;

/// The replica is only worth testing against if it really IS your production schema.
#[tokio::test]
async fn replica_matches_production_columns() {
    let Some(db) = raw_baseline().await else { return };
    let actual: Value = sqlx::query_scalar(DIGEST).fetch_one(&db.su).await.unwrap();
    let expected: BTreeMap<String, String> =
        serde_json::from_str(include_str!("fixtures/production_columns.json")).unwrap();
    let actual: BTreeMap<String, String> = serde_json::from_value(actual).unwrap();
    assert_eq!(actual, expected, "replica drifted from production_columns.json (see scripts/schema_digest.sql)");

    // plus the structural counts taken from the production dump
    let policies: i64 = sqlx::query_scalar("select count(*) from pg_policies where schemaname='public'").fetch_one(&db.su).await.unwrap();
    let rls_off: i64 = sqlx::query_scalar("select count(*) from pg_class where relnamespace='public'::regnamespace and relkind='r' and not relrowsecurity").fetch_one(&db.su).await.unwrap();
    assert_eq!((policies, rls_off), (86, 10));
    db.cleanup().await;
}

/// On Supabase a REVOKE on the http extension's functions is silently ignored (we don't own them).
/// The migration must notice, fail, and change NOTHING.
#[tokio::test]
async fn close_anonymous_access_fails_loudly_when_http_is_still_installed() {
    let Some(db) = raw_baseline().await else { return };
    let err = MIGRATOR.run(&db.app).await.expect_err("must fail while the http functions are still callable");
    let msg = err.to_string();
    assert!(msg.contains("http") && msg.contains("Extensions"), "unhelpful error: {msg}");

    let rls_on: bool = sqlx::query_scalar("select relrowsecurity from pg_class where relname='event_income' and relnamespace='public'::regnamespace")
        .fetch_one(&db.su).await.unwrap();
    assert!(!rls_on, "a failed migration must roll back completely");
    db.cleanup().await;
}

#[tokio::test]
async fn close_anonymous_access_is_safe_to_run_twice() {
    let Some(env) = setup().await else { return };
    // already applied by the template; pasting it into the SQL editor again must be harmless
    sqlx::raw_sql(include_str!("../migrations/20261111090000_close_anonymous_access.sql"))
        .execute(&env.pool).await.expect("second run");
    let policies: i64 = sqlx::query_scalar("select count(*) from pg_policies where tablename='event_income'").fetch_one(&env.pool).await.unwrap();
    let triggers: i64 = sqlx::query_scalar("select count(*) from pg_trigger where tgname='member_profiles_guard'").fetch_one(&env.pool).await.unwrap();
    assert_eq!((policies, triggers), (1, 1), "no duplicate policies or triggers");
    env.cleanup().await;
}

#[tokio::test]
async fn migration_versions_are_unique_and_never_contain_transaction_control() {
    // the sqlx error you hit: two files with the same version, and COMMIT inside a migration
    let mut seen = std::collections::HashSet::new();
    for m in MIGRATOR.iter() {
        assert!(seen.insert(m.version), "duplicate migration version {}", m.version);
        let sql = m.sql.to_lowercase();
        for forbidden in ["\nbegin;", "\ncommit;", "\nrollback;"] {
            assert!(!format!("\n{sql}").contains(forbidden), "{} must not contain `{}`: sqlx already wraps each migration in a transaction", m.description, forbidden.trim());
        }
    }
}
