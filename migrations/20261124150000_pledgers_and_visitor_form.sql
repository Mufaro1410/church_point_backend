-- Pledges can be made by a member, a ministry, or any other person/organisation (a name).
-- Giving can be attached to a pledge, so pledges by ministries and organisations get a progress too.
-- Visitors can register themselves through a public form reached by a QR code.

alter table public.pledges alter column member_id drop not null;
alter table public.pledges add column if not exists ministry_id uuid references public.ministries(id) on delete cascade;
alter table public.pledges add column if not exists pledger_name text;
do $$ begin
  if not exists (select 1 from pg_constraint where conname = 'pledges_one_pledger') then
    alter table public.pledges add constraint pledges_one_pledger check (
      (num_nonnulls(member_id, ministry_id) = 1 and pledger_name is null)
      or (num_nonnulls(member_id, ministry_id) = 0 and btrim(coalesce(pledger_name, '')) <> ''));
  end if;
end $$;
create index if not exists pledges_ministry_idx on public.pledges (ministry_id);

alter table public.incomes add column if not exists pledge_id uuid references public.pledges(id) on delete set null;
create index if not exists incomes_pledge_idx on public.incomes (pledge_id) where pledge_id is not null;

alter table public.visitors add column if not exists source text not null default 'admin' check (source in ('admin', 'form'));

create table if not exists public.visitor_form_links (
    foc_id     uuid primary key references public.foc_profile(id) on delete cascade,
    token      text not null unique default replace(gen_random_uuid()::text || gen_random_uuid()::text, '-', ''),
    enabled    boolean not null default true,
    created_at timestamptz not null default now()
);
alter table public.visitor_form_links enable row level security;
revoke all on public.visitor_form_links from anon, authenticated;
