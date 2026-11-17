-- Each church connects its OWN Paynow merchant account; money goes straight to that church.
create table if not exists public.foc_payment_settings (
    foc_id              uuid primary key references public.foc_profile (id) on update cascade on delete cascade,
    provider            text    not null default 'paynow' check (provider = 'paynow'),
    integration_id      text    not null,
    integration_key_enc text    not null,          -- encrypted by the server; never sent to any browser
    merchant_email      text    not null,          -- Paynow login email (used as authemail in test mode)
    test_mode           boolean not null default true,
    category_id         uuid references public.categories (id) on update cascade on delete set null,
    method_id           uuid references public.methods (id) on update cascade on delete set null,
    enabled             boolean not null default false,
    updated_at          timestamptz not null default now()
);

create table if not exists public.payments (
    id               uuid primary key default gen_random_uuid(),
    foc_id           uuid not null references public.foc_profile (id) on update cascade on delete restrict,
    member_id        uuid references public.member_profiles (id) on update cascade on delete set null,
    amount           numeric(14,2) not null check (amount > 0),
    currency_id      uuid not null references public.currencies (id) on update cascade on delete restrict,
    purpose          text,
    method           text not null check (method in ('ecocash', 'onemoney')),
    phone            text not null,
    status           text not null default 'created' check (status in ('created','pending','paid','cancelled','failed')),
    paynow_status    text,
    paynow_reference text,
    poll_url         text,
    instructions     text,
    error            text,
    income_id        uuid unique references public.incomes (id) on update cascade on delete set null,
    created_at       timestamptz not null default now(),
    updated_at       timestamptz not null default now()
);

create index if not exists payments_foc_created_idx on public.payments (foc_id, created_at desc);
create index if not exists payments_member_idx on public.payments (member_id);

alter table public.foc_payment_settings enable row level security;
alter table public.payments enable row level security;
revoke all on public.foc_payment_settings from anon, authenticated;
revoke all on public.payments from anon, authenticated;
