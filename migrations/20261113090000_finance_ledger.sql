-- Phase 3: a trustworthy finance ledger.
--   * money is numeric(14,2), never float
--   * exchange rates are per FOC and SNAPSHOTTED on every record (editing a rate never rewrites history)
--   * records are voided with a reason, never deleted; every change is written to finance_audit
--   * event income is merged into incomes (one ledger, one report)
-- Rate meaning: units of the record's currency per 1 USD (USD = 1).  amount_usd = amount / exchange_rate.

create or replace function pg_temp.add_constraint(p_table regclass, p_name text, p_def text) returns void
language plpgsql as $$
begin
  if not exists (select 1 from pg_constraint where conrelid = p_table and conname = p_name) then
    execute format('alter table %s add constraint %I %s', p_table, p_name, p_def);
  end if;
end $$;

-- 1. PER-FOC EXCHANGE RATES -----------------------------------------------------------------------
alter table public.foc_currencies add column if not exists rate numeric(18,6);
select pg_temp.add_constraint('public.foc_currencies', 'foc_currencies_rate_positive', 'check (rate is null or rate > 0)');

update public.foc_currencies fc set rate = round(c.rate::numeric, 6)
  from public.currencies c
 where c.id = fc.currency_id and fc.rate is null and c.rate is not null and c.rate > 0;
update public.foc_currencies fc set rate = 1
  from public.currencies c where c.id = fc.currency_id and upper(c.name) = 'USD';

-- 2. MONEY AS numeric, PLUS THE COLUMNS A LEDGER NEEDS --------------------------------------------
do $$
declare bad bigint;
begin
  select count(*) into bad from (select amount from public.incomes union all select amount from public.expenses) t where amount <= 0;
  if bad > 0 then raise exception '% income/expense rows have an amount <= 0. Correct or remove them first.', bad; end if;
end $$;

alter table public.incomes  alter column amount type numeric(14,2) using round(amount::numeric, 2);
alter table public.expenses alter column amount type numeric(14,2) using round(amount::numeric, 2);

alter table public.incomes
  add column if not exists exchange_rate numeric(18,6),
  add column if not exists recorded_by uuid references public.member_profiles (id) on update cascade on delete set null,
  add column if not exists event_id uuid references public.events (id) on update cascade on delete restrict,
  add column if not exists legacy_event_income_id uuid unique,
  add column if not exists voided_at timestamptz,
  add column if not exists voided_by uuid references public.member_profiles (id) on update cascade on delete set null,
  add column if not exists void_reason text;
alter table public.expenses
  add column if not exists exchange_rate numeric(18,6),
  add column if not exists recorded_by uuid references public.member_profiles (id) on update cascade on delete set null,
  add column if not exists voided_at timestamptz,
  add column if not exists voided_by uuid references public.member_profiles (id) on update cascade on delete set null,
  add column if not exists void_reason text;

-- generated AFTER the amount type change (a generated column blocks altering the column it uses)
alter table public.incomes  add column if not exists amount_usd numeric(14,2)
  generated always as (case when exchange_rate > 0 then round(amount / exchange_rate, 2) end) stored;
alter table public.expenses add column if not exists amount_usd numeric(14,2)
  generated always as (case when exchange_rate > 0 then round(amount / exchange_rate, 2) end) stored;

-- 3. EVENT INCOME -> INCOMES (idempotent: each legacy row is copied once) -------------------------------
insert into public.incomes (foc_id, category_id, amount, currency_id, method_id, description, date, event_id,
                            recorded_by, created_at, updated_at, exchange_rate, legacy_event_income_id)
select e.foc_id,
       coalesce(e.category_id, (select id from public.categories where lower(name) = 'event income' limit 1)),
       e.amount, e.currency_id,
       coalesce((select m.id from public.methods m where lower(m.name) = lower(e.method) limit 1),
                (select m.id from public.methods m where lower(m.name) = 'cash' limit 1)),
       nullif(e.notes, ''), e.created_at::date, e.event_id, e.recorded_by, e.created_at,
       coalesce(e.updated_at, e.created_at),
       coalesce(fc.rate, case when upper(cu.name) = 'USD' then 1 end),
       e.id
from public.event_income e
join public.currencies cu on cu.id = e.currency_id
left join public.foc_currencies fc on fc.foc_id = e.foc_id and fc.currency_id = e.currency_id
where e.amount > 0
  and not exists (select 1 from public.incomes i where i.legacy_event_income_id = e.id);

-- rows that already existed have no recorded rate: use the FOC's rate (USD = 1) where there is one
update public.incomes i set exchange_rate = coalesce(fc.rate, case when upper(cu.name) = 'USD' then 1 end)
  from public.currencies cu
  left join public.foc_currencies fc on fc.currency_id = cu.id
 where i.exchange_rate is null and cu.id = i.currency_id and (fc.foc_id = i.foc_id or upper(cu.name) = 'USD');
update public.expenses x set exchange_rate = coalesce(fc.rate, case when upper(cu.name) = 'USD' then 1 end)
  from public.currencies cu
  left join public.foc_currencies fc on fc.currency_id = cu.id
 where x.exchange_rate is null and cu.id = x.currency_id and (fc.foc_id = x.foc_id or upper(cu.name) = 'USD');

-- 4. RULES THE LEDGER ENFORCES ITSELF ---------------------------------------------------------------
do $$
declare bad bigint;
begin
  select count(*) into bad from public.incomes  where currency_id is null or category_id is null or method_id is null;
  if bad > 0 then raise exception '% income rows lack a currency, category or method. Fill them in first.', bad; end if;
  select count(*) into bad from public.expenses where currency_id is null or category_id is null or method_id is null;
  if bad > 0 then raise exception '% expense rows lack a currency, category or method. Fill them in first.', bad; end if;
end $$;

alter table public.incomes  alter column currency_id set not null, alter column category_id set not null, alter column method_id set not null;
alter table public.expenses alter column currency_id set not null, alter column category_id set not null, alter column method_id set not null;

select pg_temp.add_constraint('public.incomes',  'incomes_amount_positive',  'check (amount > 0)');
select pg_temp.add_constraint('public.expenses', 'expenses_amount_positive', 'check (amount > 0)');
select pg_temp.add_constraint('public.incomes',  'incomes_rate_positive',  'check (exchange_rate is null or exchange_rate > 0)');
select pg_temp.add_constraint('public.expenses', 'expenses_rate_positive', 'check (exchange_rate is null or exchange_rate > 0)');
select pg_temp.add_constraint('public.incomes',  'incomes_void_consistent',
  'check ((voided_at is null and void_reason is null) or (voided_at is not null and void_reason is not null))');
select pg_temp.add_constraint('public.expenses', 'expenses_void_consistent',
  'check ((voided_at is null and void_reason is null) or (voided_at is not null and void_reason is not null))');

create index if not exists incomes_event_idx on public.incomes (event_id) where event_id is not null;

-- 5. AUDIT TRAIL (replaces contribution_changes, whose int4 id column could never hold a uuid) ----------
create table if not exists public.finance_audit (
  id          uuid primary key default gen_random_uuid(),
  foc_id      uuid not null references public.foc_profile (id) on update cascade on delete restrict,
  record_type text not null check (record_type in ('income', 'expense')),
  record_id   uuid not null,
  action      text not null check (action in ('create', 'update', 'void')),
  changed_by  uuid references public.member_profiles (id) on update cascade on delete set null,
  reason      text,
  before_data jsonb,
  after_data  jsonb,
  created_at  timestamptz not null default now()
);
create index if not exists finance_audit_foc_created_idx on public.finance_audit (foc_id, created_at desc);
create index if not exists finance_audit_record_idx on public.finance_audit (record_type, record_id);
alter table public.finance_audit enable row level security;      -- no policies: only the server reads or writes it
revoke all on public.finance_audit from anon, authenticated;

do $$
begin
  if to_regclass('public.contribution_changes') is not null then
    if exists (select 1 from public.contribution_changes) then
      raise exception 'contribution_changes has rows; export them before this migration drops it';
    end if;
    drop table public.contribution_changes;
  end if;
end $$;

-- 6. VERIFY -----------------------------------------------------------------------------------------
do $$
declare bad text;
begin
  select string_agg(table_name || '.' || column_name, ', ') into bad from information_schema.columns
   where table_schema = 'public' and table_name in ('incomes', 'expenses') and column_name = 'amount' and data_type <> 'numeric';
  if bad is not null then raise exception 'money is still not numeric: %', bad; end if;

  if exists (select 1 from public.event_income e where e.amount > 0
             and not exists (select 1 from public.incomes i where i.legacy_event_income_id = e.id)) then
    raise exception 'some event_income rows were not merged into incomes';
  end if;
  if not (select relrowsecurity from pg_class where oid = 'public.finance_audit'::regclass) then
    raise exception 'finance_audit must have row level security on';
  end if;
  if has_table_privilege('anon', 'public.finance_audit', 'select') or has_table_privilege('authenticated', 'public.finance_audit', 'select') then
    raise exception 'finance_audit must not be readable by API roles';
  end if;
end $$;
