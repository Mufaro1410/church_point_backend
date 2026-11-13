# ChurchPoint Backend (Axum)

Axum API in front of the Supabase Postgres database.

## Run
1. `cp .env.example .env`, fill in DATABASE_URL, SUPABASE_JWT_SECRET, SUPABASE_URL
2. `cargo run`  ->  `curl localhost:8080/ready`

## Who can use the API
Only users with role `admin`. Ordinary members will use a separate member API (for the mobile app).
Cell leaders are paused until custom roles and permissions exist.

## Database changes = migrations (Axum owns the schema)
All schema changes live in `migrations/` and are applied with sqlx:

    cargo install sqlx-cli --no-default-features --features rustls,postgres   # once
    scripts/backup_before_migration.sh                                        # always, first
    sqlx migrate run                                                          # uses DATABASE_URL
    # then run scripts/verify_remote.sql in the Supabase SQL editor

Rules (each one was learned the hard way and is enforced by a test):
- one file per change, named `<timestamp>_<description>.sql`, versions never repeat
  (`sqlx migrate add <description>` creates it)
- NO `begin;` / `commit;` inside a file: sqlx already wraps each migration in a transaction
- never put rollback scripts in `migrations/` (they are applied too!); keep them in `sql/rollbacks/`
- migrations check their own result and fail loudly instead of silently doing nothing
- never edit a migration that has been applied; add a new one

## Tests
- `cargo test`: unit tests + "every route needs a token". No database needed.
- Database tests need a THROWAWAY Postgres with a superuser:

      docker compose up -d testdb
      TEST_DATABASE_URL=postgres://postgres:postgres@localhost:55432/postgres cargo test

  Without `TEST_DATABASE_URL` the database tests are skipped, and a skipped test still prints as "ok".
  They run against `tests/fixtures/baseline*.sql`, a replica of the production schema (columns, constraints,
  functions, triggers and all 86 RLS policies), as a non-superuser role like Supabase's `postgres`, then
  apply `migrations/`. `replica_matches_production_columns` keeps the replica honest: re-run
  `scripts/schema_digest.sql` on Supabase whenever the real schema changes and save the result as
  `tests/fixtures/production_columns.json`.

## Known open items (see tests/lockdown_tests.rs)
Anonymous visitors can still read `member_profiles` (phones, addresses) and `foc_profile.payment_details`.
They are closed in a later phase, once the web and mobile apps stop reading those tables directly.
