-- Serving capacities: specific jobs a member can serve in on an event day (Prayer, Parking, Sound desk…).
-- Each church (FOC) keeps its own list, managed by its admins in the web app.
create table if not exists public.serving_capacities (
    id         uuid primary key default gen_random_uuid(),
    foc_id     uuid not null references public.foc_profile (id) on update cascade on delete cascade,
    name       text not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);
create unique index if not exists serving_capacities_foc_name_uq on public.serving_capacities (foc_id, lower(name));
alter table public.serving_capacities enable row level security;
revoke all on public.serving_capacities from anon, authenticated;

-- Push notification choices. A member with no row here has everything switched on.
create table if not exists public.member_notification_prefs (
    member_id     uuid primary key references public.member_profiles (id) on update cascade on delete cascade,
    enabled       boolean not null default true,
    announcements boolean not null default true,
    events        boolean not null default true,
    updated_at    timestamptz not null default now()
);
alter table public.member_notification_prefs enable row level security;
revoke all on public.member_notification_prefs from anon, authenticated;

-- Remembers which event reminders were already sent, so nobody gets the same one twice.
create table if not exists public.event_reminders_sent (
    event_id uuid not null references public.events (id) on delete cascade,
    kind     text not null,
    sent_at  timestamptz not null default now(),
    primary key (event_id, kind)
);
alter table public.event_reminders_sent enable row level security;
revoke all on public.event_reminders_sent from anon, authenticated;
