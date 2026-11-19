-- Membership lifecycle: finance payer snapshot, membership history, several serving roles, account deletion log.

-- 1. FINANCE PAYER SNAPSHOT ------------------------------------------------------------------------
-- The payer's name as it was when the money was recorded, so a ledger still reads correctly after the
-- member transfers, is released or deletes their account. Only finance/admin queries expose it.
alter table public.incomes  add column if not exists payer_name text;
alter table public.payments add column if not exists payer_name text;

update public.incomes i set payer_name = nullif(btrim(concat_ws(' ', m.first_name, m.last_name)), '')
  from public.member_profiles m where m.id = i.member_id and i.payer_name is null;
update public.payments p set payer_name = nullif(btrim(concat_ws(' ', m.first_name, m.last_name)), '')
  from public.member_profiles m where m.id = p.member_id and p.payer_name is null;

create or replace function public.snapshot_payer_name() returns trigger language plpgsql as $$
begin
  if new.member_id is not null and new.payer_name is null then
    select nullif(btrim(concat_ws(' ', m.first_name, m.last_name)), '') into new.payer_name
      from public.member_profiles m where m.id = new.member_id;
  end if;
  return new;
end $$;
drop trigger if exists incomes_snapshot_payer on public.incomes;
create trigger incomes_snapshot_payer before insert on public.incomes for each row execute function public.snapshot_payer_name();
drop trigger if exists payments_snapshot_payer on public.payments;
create trigger payments_snapshot_payer before insert on public.payments for each row execute function public.snapshot_payer_name();

-- 2. MEMBERSHIP HISTORY ------------------------------------------------------------------------------
create table if not exists public.membership_history (
    id         uuid primary key default gen_random_uuid(),
    member_id  uuid not null references public.member_profiles (id) on update cascade on delete cascade,
    foc_id     uuid not null references public.foc_profile (id) on update cascade on delete cascade,
    started_at timestamptz not null default now(),
    ended_at   timestamptz,
    end_reason text
);
create index if not exists membership_history_member_idx on public.membership_history (member_id, started_at);
create unique index if not exists membership_history_one_open on public.membership_history (member_id) where ended_at is null;
alter table public.membership_history enable row level security;
revoke all on public.membership_history from anon, authenticated;

insert into public.membership_history (member_id, foc_id, started_at)
select m.id, m.foc_id, coalesce(m.created_at, now()) from public.member_profiles m
 where m.foc_id is not null and m.deleted_at is null
   and not exists (select 1 from public.membership_history h where h.member_id = m.id);

create or replace function public.track_membership() returns trigger language plpgsql as $$
begin
  if tg_op = 'INSERT' then
    if new.foc_id is not null then
      insert into public.membership_history (member_id, foc_id) values (new.id, new.foc_id);
    end if;
    return new;
  end if;
  if new.foc_id is distinct from old.foc_id then
    update public.membership_history set ended_at = now(),
           end_reason = case when new.deleted_at is not null then 'deleted'
                             when new.foc_id is null then 'released' else 'moved' end
     where member_id = new.id and ended_at is null;
    if new.foc_id is not null then
      insert into public.membership_history (member_id, foc_id) values (new.id, new.foc_id);
    end if;
  end if;
  return new;
end $$;
drop trigger if exists member_profiles_track_membership on public.member_profiles;
create trigger member_profiles_track_membership after insert or update of foc_id on public.member_profiles
  for each row execute function public.track_membership();

-- 3. SEVERAL SERVING ROLES PER EVENT -----------------------------------------------------------------
alter table public.event_serving drop constraint if exists event_serving_event_profile_unique;
create unique index if not exists event_serving_event_profile_capacity_uq
  on public.event_serving (event_id, profile_id, lower(coalesce(capacity, '')));

-- 4. ACCOUNT DELETION LOG (no personal data) ---------------------------------------------------------
create table if not exists public.account_deletions (
    id           uuid primary key default gen_random_uuid(),
    member_id    uuid not null references public.member_profiles (id) on update cascade on delete cascade,
    foc_id       uuid references public.foc_profile (id) on update cascade on delete set null,
    reason       text not null,
    auth_user_id uuid,          -- cleared once the login is removed; lets a failed removal be retried
    deleted_at   timestamptz not null default now()
);
alter table public.account_deletions enable row level security;
revoke all on public.account_deletions from anon, authenticated;
