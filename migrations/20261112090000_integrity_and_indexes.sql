-- Phase 2a: stop deletes from silently destroying data, make identity unambiguous, add indexes.
-- Foreign keys are found by (table, column), not by name, so this works whatever they are called.
-- Pre-checks stop with a clear message if existing data would violate a new rule. Verified at the end.

create or replace function pg_temp.set_fk(p_table regclass, p_column text, p_ref regclass, p_name text, p_on_delete text)
returns void language plpgsql as $$
declare att smallint; c record;
begin
  select attnum into att from pg_attribute where attrelid = p_table and attname = p_column and not attisdropped;
  for c in select conname from pg_constraint where conrelid = p_table and contype = 'f' and conkey = array[att] loop
    execute format('alter table %s drop constraint %I', p_table, c.conname);
  end loop;
  execute format('alter table %s add constraint %I foreign key (%I) references %s (id) on update cascade on delete %s',
                 p_table, p_name, p_column, p_ref, p_on_delete);
end $$;

-- 1. DELETE RULES ---------------------------------------------------------------------------------
-- Deleting a cell group used to delete every member in it (and their prayers).
select pg_temp.set_fk('public.member_profiles', 'cell_group_id', 'public.cell_groups', 'member_profiles_cell_group_id_fkey', 'set null');
-- Deleting a FOC used to delete all its members.
select pg_temp.set_fk('public.member_profiles', 'foc_id', 'public.foc_profile', 'member_profiles_foc_id_fkey', 'restrict');

-- Money must never vanish because something it points to was deleted.
select pg_temp.set_fk('public.incomes',  'foc_id',      'public.foc_profile', 'incomes_foc_id_fkey',      'restrict');
select pg_temp.set_fk('public.incomes',  'currency_id', 'public.currencies',  'incomes_currency_id_fkey', 'restrict');
select pg_temp.set_fk('public.incomes',  'category_id', 'public.categories',  'incomes_category_id_fkey', 'restrict');
select pg_temp.set_fk('public.incomes',  'method_id',   'public.methods',     'incomes_method_id_fkey',   'restrict');
select pg_temp.set_fk('public.expenses', 'foc_id',      'public.foc_profile', 'expenses_foc_id_fkey',      'restrict');
select pg_temp.set_fk('public.expenses', 'ministry_id', 'public.ministries',  'expenses_ministry_id_fkey', 'restrict');
select pg_temp.set_fk('public.expenses', 'currency_id', 'public.currencies',  'expenses_currency_id_fkey', 'restrict');
select pg_temp.set_fk('public.expenses', 'category_id', 'public.categories',  'expenses_category_id_fkey', 'restrict');
select pg_temp.set_fk('public.expenses', 'method_id',   'public.methods',     'expenses_method_id_fkey',   'restrict');
-- Deleting an event used to delete its income.
select pg_temp.set_fk('public.event_income', 'event_id',    'public.events',      'event_income_event_id_fkey',    'restrict');
select pg_temp.set_fk('public.event_income', 'foc_id',      'public.foc_profile', 'event_income_foc_id_fkey',      'restrict');
select pg_temp.set_fk('public.event_income', 'category_id', 'public.categories',  'event_income_category_id_fkey', 'restrict');

-- 2. IDENTITY -------------------------------------------------------------------------------------
do $$
declare bad text;
begin
  select string_agg(user_id::text, ', ') into bad from (
    select user_id from public.member_profiles where user_id is not null group by 1 having count(*) > 1) d;
  if bad is not null then raise exception 'member_profiles has several profiles for one login (user_id): %. Merge or delete the extras first.', bad; end if;

  select string_agg(e, ', ') into bad from (
    select lower(email) as e from public.member_profiles where email is not null and deleted_at is null group by 1 having count(*) > 1) d;
  if bad is not null then raise exception 'member_profiles has duplicate emails: %. Fix them first.', bad; end if;

  select string_agg(distinct role, ', ') into bad from public.member_profiles
   where role not in ('member', 'admin', 'cell_leader', 'super_admin');
  if bad is not null then raise exception 'member_profiles.role has unexpected values: %. Allowed: member, admin, cell_leader, super_admin.', bad; end if;

  if not exists (select 1 from pg_constraint where conname = 'member_profiles_user_id_key') then
    alter table public.member_profiles add constraint member_profiles_user_id_key unique (user_id);
  end if;
  if not exists (select 1 from pg_constraint where conname = 'member_profiles_role_check') then
    alter table public.member_profiles add constraint member_profiles_role_check
      check (role in ('member', 'admin', 'cell_leader', 'super_admin'));
  end if;
end $$;
create unique index if not exists member_profiles_email_unique_ci
  on public.member_profiles (lower(email)) where email is not null and deleted_at is null;

-- 3. INDEXES (Postgres does not index foreign-key columns by itself) ------------------------------------
create index if not exists events_foc_date_idx           on public.events (foc_id, date desc);
create index if not exists events_church_date_idx        on public.events (church_id, date desc);
create index if not exists events_province_date_idx      on public.events (province_id, date desc) where province_id is not null;
create index if not exists incomes_foc_date_idx          on public.incomes (foc_id, date desc);
create index if not exists incomes_member_idx            on public.incomes (member_id);
create index if not exists incomes_category_idx          on public.incomes (category_id);
create index if not exists incomes_currency_idx          on public.incomes (currency_id);
create index if not exists incomes_method_idx            on public.incomes (method_id);
create index if not exists expenses_foc_date_idx         on public.expenses (foc_id, date desc);
create index if not exists expenses_ministry_idx         on public.expenses (ministry_id);
create index if not exists expenses_category_idx         on public.expenses (category_id);
create index if not exists expenses_currency_idx         on public.expenses (currency_id);
create index if not exists expenses_method_idx           on public.expenses (method_id);
create index if not exists prayers_foc_created_idx       on public.prayers (foc_id, created_at desc);
create index if not exists prayers_profile_idx           on public.prayers (profile_id);
create index if not exists sermons_foc_date_idx          on public.sermons (foc_id, date desc);
create index if not exists cell_groups_foc_idx           on public.cell_groups (foc_id);
create index if not exists cell_meetings_group_date_idx  on public.cell_meetings (cell_group_id, date desc);
create index if not exists member_profiles_cell_group_idx on public.member_profiles (cell_group_id);
create index if not exists foc_profile_church_idx        on public.foc_profile (church_id);
create index if not exists foc_profile_province_idx      on public.foc_profile (province_id);
create index if not exists province_church_idx           on public.province (church_id);
create index if not exists courses_church_idx            on public.courses (church_id);

-- 4. VERIFY ---------------------------------------------------------------------------------------
do $$
declare r record; actual "char";
begin
  for r in select * from (values
      ('member_profiles','cell_group_id','n'), ('member_profiles','foc_id','r'),
      ('incomes','foc_id','r'), ('incomes','currency_id','r'), ('incomes','category_id','r'), ('incomes','method_id','r'),
      ('expenses','foc_id','r'), ('expenses','ministry_id','r'), ('expenses','currency_id','r'),
      ('expenses','category_id','r'), ('expenses','method_id','r'),
      ('event_income','event_id','r'), ('event_income','foc_id','r'), ('event_income','category_id','r')
    ) as t(tbl, col, expected)
  loop
    select c.confdeltype into actual
    from pg_constraint c join pg_attribute a on a.attrelid = c.conrelid and a.attnum = any (c.conkey)
    where c.conrelid = ('public.' || r.tbl)::regclass and c.contype = 'f' and a.attname = r.col;
    if actual is distinct from r.expected::"char" then
      raise exception 'foreign key %.% has delete rule % (expected %)', r.tbl, r.col, actual, r.expected;
    end if;
  end loop;
  if not exists (select 1 from pg_constraint where conname = 'member_profiles_user_id_key')
     or not exists (select 1 from pg_constraint where conname = 'member_profiles_role_check')
     or not exists (select 1 from pg_indexes where indexname = 'member_profiles_email_unique_ci') then
    raise exception 'identity constraints were not created';
  end if;
end $$;
