-- Test fixture: the production tables the Axum code touches (copied from the real DDL).
create table church(id uuid primary key default gen_random_uuid(), name text);
create table province(id uuid primary key default gen_random_uuid(), church_id uuid references church(id), name text);
create table foc_profile(id uuid primary key default gen_random_uuid(), church_id uuid references church(id),
  province_id uuid references province(id), name text);
create table member_profiles(id uuid primary key default gen_random_uuid(), user_id uuid,
  foc_id uuid references foc_profile(id), email text, role text, first_name text, last_name text,
  status text, deleted_at timestamptz, updated_at timestamptz);

create table sermons(
  id uuid primary key default gen_random_uuid(), title text not null, preacher text not null, date date not null,
  scripture text, sermon_resources_id uuid, summary text, content text not null,
  foc_id uuid not null references foc_profile(id) on delete cascade, tags text[],
  profile_id uuid references member_profiles(id) on update cascade on delete restrict,
  updated_at timestamptz not null default now(), created_at timestamptz not null default now(),
  audio_url text, youtube_link text, facebook_link text, instagram_link text, twitter_link text, website_link text);

create table events (
  id uuid not null default gen_random_uuid(), title text not null, date date not null,
  start_time time with time zone not null, end_time time with time zone not null,
  location text not null, description text null, foc_id uuid null,
  updated_at timestamp with time zone null, created_at timestamp with time zone not null default now(),
  rsvp jsonb null, "isRecurring" text null, attendance jsonb null default '{}'::jsonb,
  qr_code text null, qr_generated_at timestamp with time zone null, sermon_id uuid null,
  manual_stats jsonb null default '{"arena": 0, "males": 0, "females": 0, "children": 0, "visitors": 0, "alterCall": 0, "salvation": 0, "reassurance": 0}'::jsonb,
  scope text not null default 'foc'::text, church_id uuid not null, province_id uuid null,
  constraint events_pkey primary key (id),
  constraint events_foc_id_fkey foreign key (foc_id) references foc_profile (id) on update cascade on delete cascade,
  constraint events_church_id_fkey foreign key (church_id) references church (id) on update cascade on delete cascade,
  constraint events_province_id_fkey foreign key (province_id) references province (id) on update cascade on delete cascade,
  constraint events_sermon_id_fkey foreign key (sermon_id) references sermons (id) on update cascade on delete set null,
  constraint events_scope_values check (scope = any (array['church','province','foc'])),
  constraint events_scope_check check (
    ((scope = 'church') and (province_id is null) and (foc_id is null))
    or ((scope = 'province') and (province_id is not null) and (foc_id is null))
    or ((scope = 'foc') and (foc_id is not null))));

create function get_visible_events(p_foc_id uuid, p_start date, p_end date) returns setof events language sql as $$
  select e.* from events e join foc_profile f on f.id = p_foc_id
  where e.date between p_start and p_end
    and ((e.scope='church' and e.church_id=f.church_id) or (e.scope='province' and e.province_id=f.province_id)
      or (e.scope='foc' and e.foc_id=f.id)) $$;

create table prayers(
  id uuid primary key default gen_random_uuid(), title text not null, content text not null,
  is_anonymous boolean not null default false,
  profile_id uuid not null references member_profiles(id) on update cascade on delete cascade,
  date date not null, created_at timestamptz not null default now(), support_count integer,
  foc_id uuid not null references foc_profile(id) on update cascade on delete cascade,
  updated_at timestamptz, reactions text[]);

create table testimonies(
  id uuid primary key default gen_random_uuid(), profile_id uuid,
  foc_id uuid not null references foc_profile(id) on delete cascade,
  title varchar not null, situation text not null, intervention text not null, outcome text not null,
  is_anonymous boolean default false, author varchar not null,
  status varchar default 'pending', reactions jsonb default '[]'::jsonb,
  date date default current_date, created_at timestamptz default now(), updated_at timestamptz default now(),
  constraint testimonies_identity_check check (profile_id is not null or author is not null),
  constraint testimonies_intervention_length check (char_length(intervention) <= 200),
  constraint testimonies_outcome_length check (char_length(outcome) <= 150),
  constraint testimonies_situation_length check (char_length(situation) <= 150),
  constraint testimonies_status_check check (status::text = any (array['pending','approved','rejected'])));

create table announcements(
  id uuid primary key default gen_random_uuid(), title text not null, content text not null,
  foc_id uuid references foc_profile(id) on update cascade on delete cascade,
  priority text default 'medium', category text, start_date date not null, end_date date,
  is_active boolean default true, created_by uuid, created_at timestamptz not null default now(),
  updated_at timestamptz, scope text not null default 'foc',
  church_id uuid not null references church(id) on update cascade on delete cascade,
  province_id uuid references province(id) on update cascade on delete cascade,
  constraint announcements_priority_check check (priority = any (array['low','medium','high','urgent'])),
  constraint announcements_scope_check check (
    ((scope = 'church') and (province_id is null) and (foc_id is null))
    or ((scope = 'province') and (province_id is not null) and (foc_id is null))
    or ((scope = 'foc') and (foc_id is not null))),
  constraint announcements_scope_values check (scope = any (array['church','province','foc'])));
