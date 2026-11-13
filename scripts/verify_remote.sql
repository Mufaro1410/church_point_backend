-- Run in the Supabase SQL editor. Every row should say ok = true.
select 'RLS on for the formerly open tables' as check,
       bool_and(relrowsecurity) as ok
from pg_class
where relnamespace = 'public'::regnamespace
  and relname in ('cell_meetings','event_income','family_relationships','foc_categories','foc_currencies',
                  'foc_methods','ministry_leaders','province','_sqlx_migrations')
union all
select 'privilege guard trigger exists', count(*) = 1 from pg_trigger where tgname = 'member_profiles_guard'
union all
select 'http extension is gone (disable it in Dashboard > Database > Extensions)',
       not exists (select 1 from pg_extension where extname = 'http')
union all
select 'anon cannot execute any http function',
       coalesce(bool_and(not has_function_privilege('anon', p.oid, 'execute')), true)
from pg_proc p where p.pronamespace = 'public'::regnamespace and p.proname like 'http%'
union all
select 'migrations applied, none failed', coalesce(bool_and(success), false) from _sqlx_migrations
union all
select 'migration versions are the new timestamps (not 0)', coalesce(bool_and(version > 20000000000000), false) from _sqlx_migrations;
