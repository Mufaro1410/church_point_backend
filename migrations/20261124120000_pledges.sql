-- Pledges: a member promises an amount to a fund (an income category) in one currency.
-- Progress is never stored: it is added up from the member's non-voided giving in that category and currency.

create table if not exists public.pledges (
    id          uuid primary key default gen_random_uuid(),
    foc_id      uuid not null references public.foc_profile(id) on delete cascade,
    member_id   uuid not null references public.member_profiles(id) on delete cascade,
    category_id uuid not null references public.categories(id) on delete restrict,
    currency_id uuid not null references public.currencies(id) on delete restrict,
    amount      numeric(14,2) not null check (amount > 0),
    start_date  date not null default current_date,
    end_date    date,
    status      text not null default 'active' check (status in ('active', 'cancelled')),
    note        text,
    created_by  uuid references public.member_profiles(id) on delete set null,
    created_at  timestamptz not null default now(),
    updated_at  timestamptz not null default now(),
    check (end_date is null or end_date >= start_date)
);
create index if not exists pledges_foc_idx on public.pledges (foc_id, status);
create index if not exists pledges_member_idx on public.pledges (member_id);

alter table public.pledges enable row level security;
revoke all on public.pledges from anon, authenticated;
