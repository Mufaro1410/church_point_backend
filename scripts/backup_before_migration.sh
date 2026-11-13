#!/usr/bin/env bash
# Backs up what you would lose if a migration went wrong:
#   - the whole `public` schema (structure AND data)
#   - the login accounts (auth.users / auth.identities), so people keep their passwords
# Storage FILES (sermon audio, logos) are separate: copy them from Dashboard > Storage if they matter.
# Needs pg_dump (PostgreSQL client tools, version >= your Supabase server version).
set -euo pipefail
cd "$(dirname "$0")/.."

DB_URL="${DATABASE_URL:-$(grep -E '^DATABASE_URL=' .env 2>/dev/null | head -1 | cut -d= -f2- || true)}"
[ -n "$DB_URL" ] || { echo "Set DATABASE_URL (or put it in .env)"; exit 1; }

STAMP=$(date +%Y%m%d-%H%M%S)
mkdir -p backups

pg_dump "$DB_URL" --format=custom --no-owner --no-privileges --schema=public \
  --file "backups/public-$STAMP.dump"
pg_dump "$DB_URL" --data-only --no-owner --no-privileges \
  --table=auth.users --table=auth.identities \
  --file "backups/auth-accounts-$STAMP.sql"

echo
ls -lh "backups/public-$STAMP.dump" "backups/auth-accounts-$STAMP.sql"
cat <<TXT

Restore tips (best into a fresh Supabase project, which already has the auth schema):
  psql "<target url>" -f backups/auth-accounts-$STAMP.sql      # accounts first
  pg_restore --no-owner --dbname "<target url>" backups/public-$STAMP.dump
  # "schema public already exists" errors are normal. Tested: data restores correctly.
Keep the backups/ folder private: it contains personal data and password hashes.
TXT
