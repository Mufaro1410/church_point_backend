-- Content moderation for member posts (prayers and testimonies): reports, blocks, and terms acceptance.

alter table public.member_profiles add column if not exists terms_accepted_at timestamptz;

create table if not exists public.content_reports (
    id            uuid primary key default gen_random_uuid(),
    foc_id        uuid not null references public.foc_profile(id) on delete cascade,
    reporter_id   uuid references public.member_profiles(id) on delete set null,
    kind          text not null check (kind in ('prayer', 'testimony')),
    content_id    uuid not null,
    author_id     uuid references public.member_profiles(id) on delete set null,
    excerpt       text,
    reason        text,
    status        text not null default 'open' check (status in ('open', 'removed', 'removed_and_ejected', 'dismissed')),
    created_at    timestamptz not null default now(),
    handled_at    timestamptz,
    handled_by    uuid references public.member_profiles(id) on delete set null
);
create index if not exists content_reports_open_idx on public.content_reports (foc_id, status, created_at);
create index if not exists content_reports_content_idx on public.content_reports (kind, content_id);
-- one report per person per post
create unique index if not exists content_reports_once on public.content_reports (reporter_id, kind, content_id) where reporter_id is not null;

create table if not exists public.member_blocks (
    id          uuid primary key default gen_random_uuid(),
    blocker_id  uuid not null references public.member_profiles(id) on delete cascade,
    blocked_id  uuid not null references public.member_profiles(id) on delete cascade,
    -- people who blocked someone by tapping an anonymous post must not learn who it is
    was_anonymous boolean not null default false,
    created_at  timestamptz not null default now(),
    unique (blocker_id, blocked_id),
    check (blocker_id <> blocked_id)
);

alter table public.content_reports enable row level security;
alter table public.member_blocks enable row level security;
revoke all on public.content_reports, public.member_blocks from anon, authenticated;
