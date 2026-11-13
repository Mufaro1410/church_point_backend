-- Undo 000_emergency_close_anon.sql (only if something broke; it re-opens the holes).
begin;
drop trigger if exists member_profiles_guard on public.member_profiles;
drop function if exists public.guard_member_profiles_write();

do $$
declare t text;
begin
  foreach t in array array[
    'cell_meetings', 'event_income', 'family_relationships', 'foc_categories',
    'foc_currencies', 'foc_methods', 'foc_ministries', 'ministry_leaders'
  ] loop
    execute format('drop policy if exists %I on public.%I', t || '_authenticated_all', t);
    execute format('alter table public.%I disable row level security', t);
  end loop;
end $$;
drop policy if exists province_select_public on public.province;
drop policy if exists province_write_authenticated on public.province;
alter table public.province disable row level security;
alter table public._sqlx_migrations disable row level security;

do $$
declare f record;
begin
  for f in
    select p.oid::regprocedure as sig from pg_proc p
    where p.pronamespace = 'public'::regnamespace
      and p.proname in ('http','http_get','http_post','http_put','http_patch','http_delete','http_head',
                        'http_header','http_list_curlopt','http_reset_curlopt','http_set_curlopt',
                        'urlencode','bytea_to_text','text_to_bytea')
  loop
    execute format('grant execute on function %s to public, anon, authenticated', f.sig);
  end loop;
end $$;
commit;
