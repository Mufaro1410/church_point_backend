-- Who served at an event, and in which role (the web "Services" panel). Safe to run whether or not
-- the table already exists: missing columns are added, nothing is dropped.
create table if not exists public.event_services (
  id uuid primary key default gen_random_uuid(),
  event_id uuid not null references public.events (id) on update cascade on delete cascade,
  member_id uuid not null references public.member_profiles (id) on update cascade on delete cascade,
  service_role text not null,
  created_at timestamptz not null default now()
);
alter table public.event_services
  add column if not exists service_domain text,
  add column if not exists marked_by uuid references public.member_profiles (id) on update cascade on delete set null,
  add column if not exists source text not null default 'manual',
  add column if not exists notes text,
  add column if not exists updated_at timestamptz not null default now();

do $$ begin
  if not exists (select 1 from pg_constraint where conname = 'event_services_source_check') then
    alter table public.event_services add constraint event_services_source_check
      check (source in ('self', 'manual', 'system'));
  end if;
  create unique index if not exists event_services_unique_role on public.event_services (event_id, member_id, service_role);
exception when unique_violation then
  raise notice 'event_services has duplicate rows; unique index skipped - clean them and re-create it';
end $$;
create index if not exists event_services_event_idx on public.event_services (event_id);

-- Only Axum reaches this table: no direct API access.
alter table public.event_services enable row level security;
revoke all on public.event_services from anon, authenticated;
