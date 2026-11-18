-- Push notification tokens for the mobile app. Kept apart from member_profiles so admin member lists never carry them.
create table if not exists public.member_push_tokens (
    member_id  uuid primary key references public.member_profiles (id) on update cascade on delete cascade,
    token      text not null,
    updated_at timestamptz not null default now()
);
alter table public.member_push_tokens enable row level security;
revoke all on public.member_push_tokens from anon, authenticated;
