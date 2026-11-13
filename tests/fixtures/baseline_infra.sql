-- =====================================================================================
-- baseline.sql: a faithful REPLICA of the current Supabase `public` schema.
-- Reconstructed from the schema dumps (columns, constraints, indexes, triggers, function
-- bodies, RLS flags and all 86 policies). It is the "before" state that migrations/ changes.
-- Parts the dump did not show are marked ASSUMED. Run scripts/schema_digest.sql against
-- Supabase and compare with tests/fixtures/schema_digest.txt to catch any drift.
-- =====================================================================================

-- ---- Supabase infrastructure stand-ins (not part of your schema) ----
do $$ begin create role anon nologin;          exception when duplicate_object then null; end $$;
do $$ begin create role authenticated nologin; exception when duplicate_object then null; end $$;
do $$ begin create role service_role nologin bypassrls; exception when duplicate_object then null; end $$;

-- `app_admin` mimics Supabase's `postgres` role: NOT a superuser (so migrations cannot rely on superuser
-- powers or revoke privileges on objects they do not own), but it bypasses RLS and owns the schema objects.
do $$ begin create role app_admin nologin bypassrls; exception when duplicate_object then null; end $$;
grant create, usage on schema public to app_admin;

create schema if not exists auth;
create table auth.users (
  id uuid primary key default gen_random_uuid(),
  email text, phone text, raw_user_meta_data jsonb, created_at timestamptz default now());
create function auth.uid() returns uuid language sql stable as
  $$ select nullif(current_setting('request.jwt.claim.sub', true), '')::uuid $$;
grant usage on schema auth to anon, authenticated, service_role, app_admin;
grant select, references on auth.users to app_admin;

create schema if not exists realtime;
create table realtime.broadcast_log (id bigserial primary key, topic text, event text, operation text,
  tbl text, new_row jsonb, old_row jsonb);
-- Records what each trigger would broadcast, so tests can inspect it.
create function realtime.broadcast_changes(topic_name text, event_name text, operation text, table_name text,
  table_schema text, new record, old record, level text default 'ROW') returns void language plpgsql security definer as $$
begin
  insert into realtime.broadcast_log(topic, event, operation, tbl, new_row, old_row)
  values (topic_name, event_name, operation, table_name, row_to_json(new)::jsonb, row_to_json(old)::jsonb);
end $$;

grant usage on schema realtime to app_admin;

-- uuid_generate_v4(): from the uuid-ossp extension in Supabase
create function public.uuid_generate_v4() returns uuid language sql as $$ select gen_random_uuid() $$;

-- ---- the `http` extension (pgsql-http) is installed in your public schema: signature stand-ins ----
create type public.http_header as (field varchar, value varchar);
create type public.http_request as (method varchar, uri varchar, headers public.http_header[], content_type varchar, content varchar);
create type public.http_response as (status integer, content_type varchar, headers public.http_header[], content varchar);
create function public.http(request public.http_request) returns public.http_response language sql as $$ select null::public.http_response $$;
create function public.http_get(uri varchar) returns public.http_response language sql as $$ select null::public.http_response $$;
create function public.http_get(uri varchar, data jsonb) returns public.http_response language sql as $$ select null::public.http_response $$;
create function public.http_post(uri varchar, data jsonb) returns public.http_response language sql as $$ select null::public.http_response $$;
create function public.http_post(uri varchar, content varchar, content_type varchar) returns public.http_response language sql as $$ select null::public.http_response $$;
create function public.http_put(uri varchar, content varchar, content_type varchar) returns public.http_response language sql as $$ select null::public.http_response $$;
create function public.http_patch(uri varchar, content varchar, content_type varchar) returns public.http_response language sql as $$ select null::public.http_response $$;
create function public.http_delete(uri varchar) returns public.http_response language sql as $$ select null::public.http_response $$;
create function public.http_delete(uri varchar, content varchar, content_type varchar) returns public.http_response language sql as $$ select null::public.http_response $$;
create function public.http_head(uri varchar) returns public.http_response language sql as $$ select null::public.http_response $$;
create function public.http_header(field varchar, value varchar) returns public.http_header language sql as $$ select $1, $2 $$;
create function public.http_list_curlopt() returns table(curlopt text, value text) language sql as $$ select null::text, null::text where false $$;
create function public.http_reset_curlopt() returns boolean language sql as $$ select true $$;
create function public.http_set_curlopt(curlopt varchar, value varchar) returns boolean language sql as $$ select true $$;
create function public.urlencode(string bytea) returns text language sql as $$ select ''::text $$;
create function public.urlencode(string varchar) returns text language sql as $$ select string::text $$;
create function public.urlencode(data jsonb) returns text language sql as $$ select ''::text $$;
create function public.bytea_to_text(data bytea) returns text language sql as $$ select ''::text $$;
create function public.text_to_bytea(data text) returns bytea language sql as $$ select ''::bytea $$;
