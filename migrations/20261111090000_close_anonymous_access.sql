-- Closes the holes that let ANONYMOUS visitors (and ordinary members) read or change data.
-- Designed not to break logged-in apps. Safe to run more than once.
-- Every protection is VERIFIED at the end: if one silently failed (for example a REVOKE on an object
-- we do not own, which Postgres only warns about), this migration fails loudly and changes nothing.

-- 1. Tables that had Row Level Security OFF were fully readable and writable with just the anon key.
--    Turn it on; allow logged-in users only (the same access they have today, minus anonymous).
do $$
declare t text;
begin
  foreach t in array array[
    'cell_meetings', 'event_income', 'family_relationships', 'foc_categories',
    'foc_currencies', 'foc_methods', 'foc_ministries', 'ministry_leaders'
  ] loop
    execute format('alter table public.%I enable row level security', t);
    execute format('drop policy if exists %I on public.%I', t || '_authenticated_all', t);
    execute format(
      'create policy %I on public.%I for all to authenticated using (true) with check (true)',
      t || '_authenticated_all', t);
  end loop;
end $$;

-- province stays readable before login (sign-up pickers may need it); writes need a login.
alter table public.province enable row level security;
drop policy if exists province_select_public on public.province;
drop policy if exists province_write_authenticated on public.province;
create policy province_select_public on public.province for select to anon, authenticated using (true);
create policy province_write_authenticated on public.province for all to authenticated using (true) with check (true);

alter table public._sqlx_migrations enable row level security;

-- 2. The `http` extension lets anyone make your database fetch arbitrary URLs. Nothing uses it any more.
--    On Supabase its functions belong to another role, so REVOKE cannot touch them: the extension itself
--    must be removed (Dashboard > Database > Extensions > http > disable, or `drop extension http`).
do $$
begin
  if exists (select 1 from pg_extension where extname = 'http') then
    drop extension http cascade;
  end if;
end $$;

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
    execute format('revoke execute on function %s from public, anon, authenticated', f.sig);
  end loop;
end $$;

-- 3. Privilege escalation: any logged-in user could UPDATE any profile (including `role`) or INSERT a profile
--    that is already an approved admin. Block it for direct API callers. Exempt: the Axum connection and
--    SECURITY DEFINER functions (they run as a different database role) and real admins.
create or replace function public.guard_member_profiles_write() returns trigger
language plpgsql set search_path = public, pg_temp as $$
begin
  if current_user not in ('anon', 'authenticated') then
    return new;
  end if;
  if public.is_member_admin() then
    return new;
  end if;

  if tg_op = 'INSERT' then
    if new.role is distinct from 'member' or new.status is distinct from 'pending' then
      raise exception 'New profiles must have role member and status pending' using errcode = '42501';
    end if;
  else
    if new.role is distinct from old.role or new.user_id is distinct from old.user_id then
      raise exception 'You cannot change role or user_id' using errcode = '42501';
    end if;
    if new.status is distinct from old.status and new.status = 'approved' then
      raise exception 'You cannot approve a profile' using errcode = '42501';
    end if;
  end if;
  return new;
end $$;

drop trigger if exists member_profiles_guard on public.member_profiles;
create trigger member_profiles_guard
  before insert or update on public.member_profiles
  for each row execute function public.guard_member_profiles_write();

-- 4. VERIFY. Any failure aborts the whole migration with a message that says what to do.
do $$
declare bad text;
begin
  select string_agg(c.relname, ', ') into bad
  from pg_class c
  where c.relnamespace = 'public'::regnamespace
    and c.relname in ('cell_meetings','event_income','family_relationships','foc_categories','foc_currencies',
                      'foc_methods','ministry_leaders','province','_sqlx_migrations')
    and not c.relrowsecurity;
  if bad is not null then raise exception 'Row Level Security is still OFF for: %', bad; end if;

  select string_agg(p.oid::regprocedure::text, ', ') into bad
  from pg_proc p
  where p.pronamespace = 'public'::regnamespace
    and p.proname in ('http','http_get','http_post','http_put','http_patch','http_delete','http_head',
                      'http_header','http_list_curlopt','http_reset_curlopt','http_set_curlopt',
                      'urlencode','bytea_to_text','text_to_bytea')
    and (has_function_privilege('anon', p.oid, 'execute') or has_function_privilege('authenticated', p.oid, 'execute'));
  if bad is not null then
    raise exception 'These http functions can still be called by anon/authenticated: %. Disable the "http" extension in Dashboard > Database > Extensions, then run this migration again.', bad;
  end if;

  if not exists (select 1 from pg_trigger where tgname = 'member_profiles_guard') then
    raise exception 'member_profiles_guard trigger was not created';
  end if;
end $$;
