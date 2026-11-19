-- Custom roles with permissions (per church) and age groups defined by each church.

create table if not exists public.age_groups (
    id         uuid primary key default gen_random_uuid(),
    foc_id     uuid not null references public.foc_profile (id) on update cascade on delete cascade,
    name       text not null,
    min_age    integer not null check (min_age >= 0),
    max_age    integer check (max_age is null or max_age >= min_age),   -- null = and above
    sort_order integer not null default 0,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);
create unique index if not exists age_groups_foc_name_uq on public.age_groups (foc_id, lower(name));
alter table public.age_groups enable row level security;
revoke all on public.age_groups from anon, authenticated;

create table if not exists public.church_roles (
    id                   uuid primary key default gen_random_uuid(),
    foc_id               uuid not null references public.foc_profile (id) on update cascade on delete cascade,
    name                 text not null,
    description          text,
    permissions          text[] not null default '{}',
    scope_cell_group_ids uuid[] not null default '{}',   -- empty = every cell group
    scope_age_group_ids  uuid[] not null default '{}',   -- empty = every age group
    created_at           timestamptz not null default now(),
    updated_at           timestamptz not null default now()
);
create unique index if not exists church_roles_foc_name_uq on public.church_roles (foc_id, lower(name));
alter table public.church_roles enable row level security;
revoke all on public.church_roles from anon, authenticated;

create table if not exists public.member_church_roles (
    member_id   uuid not null references public.member_profiles (id) on update cascade on delete cascade,
    role_id     uuid not null references public.church_roles (id) on update cascade on delete cascade,
    assigned_at timestamptz not null default now(),
    assigned_by uuid references public.member_profiles (id) on delete set null,
    primary key (member_id, role_id)
);
create index if not exists member_church_roles_role_idx on public.member_church_roles (role_id);
alter table public.member_church_roles enable row level security;
revoke all on public.member_church_roles from anon, authenticated;
