# ChurchPoint Backend (Axum)

Axum API in front of the existing Supabase Postgres database.

## Run
1. `cp .env.example .env`, then fill in DATABASE_URL, SUPABASE_JWT_SECRET (and SUPABASE_URL)
2. `cargo run`
3. `curl localhost:8080/health`  and  `curl localhost:8080/ready`

## Database changes (run by hand in the Supabase SQL editor)
Axum never changes your schema. SQL you run yourself lives in `sql/`:
- `001_content_migration.sql`: REQUIRED before deploying the content patch. Adds nullable
  columns only (`created_by_foc_id` on events/announcements, moderation stamps on testimonies).
- `002_realtime_signal_only.sql`: PROPOSAL, optional. Stops realtime from broadcasting private rows.

## Tests
- `cargo test`: unit tests + "every route needs a token" tests. No database needed.
- Database tests (tenant isolation, anonymity, ownership). Skipped unless you point them at a
  THROWAWAY Postgres (each test creates and drops its own schema; never use production):
  `TEST_DATABASE_URL=postgres://user:pw@localhost:5432/postgres cargo test --test db_tests`
