-- Visitors (first-time guests) and a follow-up log for visitors and for members who stopped attending.

create table if not exists public.visitors (
    id                  uuid primary key default gen_random_uuid(),
    foc_id              uuid not null references public.foc_profile(id) on delete cascade,
    full_name           text not null check (char_length(btrim(full_name)) between 1 and 120),
    phone               text,
    email               text,
    visit_date          date not null default current_date,
    how_heard           text,
    notes               text,
    status              text not null default 'new' check (status in ('new', 'contacted', 'following_up', 'joined', 'closed')),
    assigned_to         uuid references public.member_profiles(id) on delete set null,
    converted_member_id uuid references public.member_profiles(id) on delete set null,
    created_by          uuid references public.member_profiles(id) on delete set null,
    created_at          timestamptz not null default now(),
    updated_at          timestamptz not null default now()
);
create index if not exists visitors_foc_idx on public.visitors (foc_id, status, visit_date desc);

create table if not exists public.follow_ups (
    id         uuid primary key default gen_random_uuid(),
    foc_id     uuid not null references public.foc_profile(id) on delete cascade,
    visitor_id uuid references public.visitors(id) on delete cascade,
    member_id  uuid references public.member_profiles(id) on delete cascade,
    kind       text not null default 'call' check (kind in ('call', 'visit', 'message', 'other')),
    note       text,
    created_by uuid references public.member_profiles(id) on delete set null,
    created_at timestamptz not null default now(),
    check ((visitor_id is not null) <> (member_id is not null))
);
create index if not exists follow_ups_visitor_idx on public.follow_ups (visitor_id, created_at desc);
create index if not exists follow_ups_member_idx on public.follow_ups (member_id, created_at desc);

alter table public.visitors enable row level security;
alter table public.follow_ups enable row level security;
revoke all on public.visitors, public.follow_ups from anon, authenticated;
