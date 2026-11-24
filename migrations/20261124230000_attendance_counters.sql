-- Attendance counters: the people numbers counted by hand at an event (males, females, ushers, livestream...).
-- A central catalogue (maintained by the platform), the counters each church uses, and the counts per event.

create table if not exists public.counter_catalogue (
    id          uuid primary key default gen_random_uuid(),
    key         text not null unique check (key ~ '^[a-z][a-z0-9_]{1,40}$'),
    label       text not null,
    -- headcount adds up to "people in the room"; the others are reported on their own
    kind        text not null check (kind in ('headcount', 'visitors', 'decisions', 'serving', 'online', 'other')),
    -- headcount counters come in counting styles; a church uses ONE style so nobody is counted twice
    style       text check (style in ('gender_children', 'age', 'venue', 'total')),
    description text,
    sort_order  integer not null default 0,
    active      boolean not null default true,
    check ((kind = 'headcount') = (style is not null))
);

create table if not exists public.foc_counters (
    foc_id     uuid not null references public.foc_profile(id) on delete cascade,
    counter_id uuid not null references public.counter_catalogue(id) on delete restrict,
    created_at timestamptz not null default now(),
    primary key (foc_id, counter_id)
);

create table if not exists public.event_counts (
    event_id   uuid not null references public.events(id) on delete cascade,
    counter_id uuid not null references public.counter_catalogue(id) on delete restrict,
    value      integer not null check (value >= 0),
    updated_by uuid references public.member_profiles(id) on delete set null,
    updated_at timestamptz not null default now(),
    primary key (event_id, counter_id)
);
create index if not exists event_counts_counter_idx on public.event_counts (counter_id);

alter table public.counter_catalogue enable row level security;
alter table public.foc_counters enable row level security;
alter table public.event_counts enable row level security;
revoke all on public.counter_catalogue, public.foc_counters, public.event_counts from anon, authenticated;

insert into public.counter_catalogue (key, label, kind, style, description, sort_order) values
 ('males',               'Males',                          'headcount', 'gender_children', null, 10),
 ('females',             'Females',                        'headcount', 'gender_children', null, 20),
 ('children',            'Children',                       'headcount', 'gender_children', null, 30),
 ('arena',               'Arena (youth)',                  'headcount', 'gender_children', 'Youth counted separately from males and females', 40),
 ('age_children',        'Children (0-12)',                'headcount', 'age', null, 110),
 ('teens',               'Teens (13-17)',                  'headcount', 'age', null, 120),
 ('youth',               'Youth (18-35)',                  'headcount', 'age', null, 130),
 ('adults',              'Adults',                         'headcount', 'age', null, 140),
 ('seniors',             'Seniors (60+)',                  'headcount', 'age', null, 150),
 ('main_hall',           'Main hall',                      'headcount', 'venue', null, 210),
 ('balcony',             'Balcony',                        'headcount', 'venue', null, 220),
 ('overflow_hall',       'Overflow hall',                  'headcount', 'venue', null, 230),
 ('outside',             'Tent or outside',                'headcount', 'venue', null, 240),
 ('childrens_church',    'Children''s church',             'headcount', 'venue', null, 250),
 ('total_attendance',    'Total attendance',               'headcount', 'total', 'One number for everyone present', 310),
 ('visitors',            'First-time visitors',            'visitors',  null, null, 410),
 ('returning_visitors',  'Returning visitors',             'visitors',  null, null, 420),
 ('special_guests',      'Special guests and VIPs',        'visitors',  null, null, 430),
 ('salvation',           'Salvation (new believers)',      'decisions', null, null, 510),
 ('alter_call',          'Altar call',                     'decisions', null, null, 520),
 ('rededication',        'Rededication',                   'decisions', null, null, 530),
 ('water_baptism',       'Baptised in water',              'decisions', null, null, 540),
 ('spirit_baptism',      'Baptised in the Holy Spirit',    'decisions', null, null, 550),
 ('healing_testimonies', 'Healing testimonies',            'decisions', null, null, 560),
 ('deliverance',         'Deliverance',                    'decisions', null, null, 570),
 ('prayer_line',         'Prayer line',                    'decisions', null, null, 580),
 ('ushers',              'Ushers',                         'serving',   null, null, 610),
 ('choir',               'Choir and praise team',          'serving',   null, null, 620),
 ('media_team',          'Media and tech team',            'serving',   null, null, 630),
 ('security',            'Security',                       'serving',   null, null, 640),
 ('sunday_school_teachers','Sunday school teachers',       'serving',   null, null, 650),
 ('nursery',             'Nursery',                        'serving',   null, null, 660),
 ('volunteers',          'Volunteers (other)',             'serving',   null, null, 670),
 ('livestream_viewers',  'Livestream viewers',             'online',    null, null, 710),
 ('online_peak',         'Online peak viewers',            'online',    null, null, 720),
 ('new_members',         'New members joined',             'other',     null, null, 810),
 ('communion',           'Communion served',               'other',     null, null, 820),
 ('ministers',           'Pastors and ministers present',  'other',     null, null, 830),
 ('vehicles',            'Vehicles in the car park',       'other',     null, null, 840)
on conflict (key) do nothing;

-- Every church starts with males, females and children, plus any counter it already has numbers for.
insert into public.foc_counters (foc_id, counter_id)
select f.id, c.id from public.foc_profile f cross join public.counter_catalogue c
where c.key in ('males', 'females', 'children')
on conflict do nothing;

-- ...and so does every church created from now on
create or replace function public.give_new_foc_default_counters() returns trigger language plpgsql as $$
begin
  insert into public.foc_counters (foc_id, counter_id)
  select new.id, c.id from public.counter_catalogue c where c.key in ('males', 'females', 'children')
  on conflict do nothing;
  return null;
end $$;
drop trigger if exists foc_default_counters on public.foc_profile;
create trigger foc_default_counters after insert on public.foc_profile
  for each row execute function public.give_new_foc_default_counters();

create temp table _legacy_keys (old text, new text) on commit drop;
insert into _legacy_keys values ('males','males'), ('females','females'), ('children','children'), ('arena','arena'),
  ('visitors','visitors'), ('alterCall','alter_call'), ('salvation','salvation'), ('reassurance','rededication');

-- numbers already entered on events move across
insert into public.event_counts (event_id, counter_id, value)
select e.id, c.id, (e.manual_stats ->> k.old)::int
from public.events e
join _legacy_keys k on (e.manual_stats ->> k.old) ~ '^[0-9]{1,9}$'
join public.counter_catalogue c on c.key = k.new
on conflict do nothing;

insert into public.foc_counters (foc_id, counter_id)
select distinct e.foc_id, ec.counter_id
from public.event_counts ec join public.events e on e.id = ec.event_id
where e.foc_id is not null and ec.value > 0
on conflict do nothing;

-- events.manual_stats stays as a read-only summary ({"males": 40, ...}) kept in step by this trigger,
-- so every screen that reads an event still sees its counts.
create or replace function public.sync_event_manual_stats() returns trigger language plpgsql as $$
declare eid uuid := coalesce(new.event_id, old.event_id);
begin
  update public.events set manual_stats = coalesce(
    (select jsonb_object_agg(c.key, x.value) from public.event_counts x join public.counter_catalogue c on c.id = x.counter_id where x.event_id = eid),
    '{}'::jsonb)
  where id = eid;
  return null;
end $$;

drop trigger if exists event_counts_sync on public.event_counts;
create trigger event_counts_sync after insert or update or delete on public.event_counts
  for each row execute function public.sync_event_manual_stats();

update public.events e set manual_stats = coalesce(
  (select jsonb_object_agg(c.key, x.value) from public.event_counts x join public.counter_catalogue c on c.id = x.counter_id where x.event_id = e.id),
  '{}'::jsonb)
where exists (select 1 from public.event_counts x where x.event_id = e.id);
