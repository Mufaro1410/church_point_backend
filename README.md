# ChurchPoint Backend (Axum)

Axum API in front of the existing Supabase Postgres database.

## Run
1. `cp .env.example .env`, then fill in DATABASE_URL and SUPABASE_JWT_SECRET
2. `cargo run`
3. `curl localhost:8080/health`  and  `curl localhost:8080/ready`
4. Call a protected route with a real token from the React app:
   `curl -H "Authorization: Bearer <access_token>" localhost:8080/api/v1/auth/me`

## Tests
`cargo test` (no database required for the current tests)
