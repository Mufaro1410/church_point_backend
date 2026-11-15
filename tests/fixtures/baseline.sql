-- baseline.sql (runs as the non-superuser role app_admin, like Supabase's postgres role).
-- See baseline_infra.sql for the header explaining what this replicates.
-- ---------------------------------- tables ----------------------------------
create table public._sqlx_migrations (
  version int8 primary key, description text not null, installed_on timestamptz not null default now(),
  success bool not null, checksum bytea not null, execution_time int8 not null);

create table public.church (
  id uuid not null default gen_random_uuid(), name text not null, address text, contact text,
  logo text not null default 'https://fcmeczxeixdczfmgdnra.supabase.co/storage/v1/object/public/churchpoint-bucket/logos/church-point-logo.png'::text,
  region_label text not null default 'Province'::text,
  constraint church_pkey primary key (id), constraint church_name_key unique (name),
  constraint church_region_label_check check (region_label = any (array['Province'::text, 'District'::text])));

create table public.province (
  id uuid not null default gen_random_uuid(), name text not null, address text, contact text, church_id uuid not null,
  constraint district_pkey primary key (id),
  constraint district_church_id_fkey foreign key (church_id) references public.church(id) on update cascade on delete cascade,
  constraint district_name_church_id_key unique (name, church_id));

create table public.foc_profile (
  id uuid not null default gen_random_uuid(), name text not null, address text not null, contact text,
  email text not null, country text, about text, website text, service_time text, social_media text[],
  church_id uuid not null,
  logo text not null default 'https://fcmeczxeixdczfmgdnra.supabase.co/storage/v1/object/public/churchpoint//churchpoint.png'::text,
  updated_at timestamptz default now(), payment_details jsonb, province_id uuid,
  constraint foc_profile_pkey primary key (id),
  constraint foc_profile_church_id_fkey foreign key (church_id) references public.church(id) on update cascade on delete cascade,
  constraint foc_profile_province_id_fkey foreign key (province_id) references public.province(id) on update cascade on delete set null);

create table public.bible_reading_plans (
  id uuid not null default gen_random_uuid(), name text not null, year int4 not null, readings jsonb not null,
  created_at timestamptz default now(), description text, user_id uuid, updated_at timestamptz not null default now(),
  deleted_at timestamptz,
  constraint bible_reading_plans_pkey primary key (id),
  constraint bible_reading_plans_user_id_fkey foreign key (user_id) references auth.users(id) on update cascade on delete set null);

create table public.ministries (
  id uuid not null default gen_random_uuid(), name text not null, description text, updated_at timestamptz,
  constraint ministry_pkey primary key (id));

create table public.courses (
  id uuid not null default gen_random_uuid(), name text not null, description text, updated_at timestamptz, church_id uuid not null,
  constraint courses_pkey primary key (id),
  constraint courses_church_id_fkey foreign key (church_id) references public.church(id) on update cascade on delete cascade);

create table public.cell_groups (
  id uuid not null default gen_random_uuid(), name text not null, foc_id uuid not null, description text,
  updated_at timestamptz default now(), start_time time, end_time time, location text, meeting_day text, leader_id uuid,
  constraint cell_group_pkey primary key (id),
  constraint cell_groups_foc_id_fkey foreign key (foc_id) references public.foc_profile(id) on update cascade on delete cascade);

create table public.member_profiles (
  id uuid not null default gen_random_uuid(), first_name text not null, last_name text not null, gender text not null,
  marital_status text, profile_image text, email text, phone text not null, address text, dob date, profession text,
  date_joined date, baptism_date date, membership_status text, cell_group_id uuid, course_ids text[], user_id uuid,
  foc_id uuid, updated_at date, created_at timestamptz, role text not null default 'member'::text, dbr_plan uuid,
  status text not null default 'pending'::text, deleted_at timestamptz, terms_accepted_at timestamptz, ministry_ids uuid[],
  constraint profiles_pkey primary key (id),
  constraint profiles_user_id_fkey foreign key (user_id) references auth.users(id) on update cascade on delete cascade,
  constraint profiles_foc_id_fkey foreign key (foc_id) references public.foc_profile(id) on update cascade on delete cascade,
  constraint profiles_cell_group_id_fkey foreign key (cell_group_id) references public.cell_groups(id) on update cascade on delete cascade,
  constraint member_profiles_dbr_plan_fkey foreign key (dbr_plan) references public.bible_reading_plans(id) on delete set null,
  constraint member_profiles_status_check check (status = any (array['pending'::text, 'approved'::text, 'rejected'::text, 'transfer_requested'::text])));

alter table public.cell_groups add constraint cell_groups_leader_id_fkey
  foreign key (leader_id) references public.member_profiles(id) on update cascade on delete set null;

-- view (definition ASSUMED from its column list)
create view public.active_member_profiles as
  select id, first_name, last_name, gender, marital_status, profile_image, email, phone, address, dob, profession,
         date_joined, baptism_date, membership_status, cell_group_id, ministry_ids, course_ids, user_id, foc_id,
         updated_at, created_at, role, dbr_plan, status, deleted_at
  from public.member_profiles where deleted_at is null;

create table public.categories (
  id uuid not null default gen_random_uuid(), name text not null, description text,
  updated_at timestamptz not null default now(), type text not null,
  constraint categories_pkey primary key (id));
create unique index categories_name_unique_ci on public.categories using btree (lower(name));

create table public.currencies (
  id uuid not null default gen_random_uuid(), name text not null, rate float8, symbol text default ''::text,
  updated_at timestamptz not null default now(), constraint currencies_pkey primary key (id));
create unique index currencies_name_unique_ci on public.currencies using btree (upper(name));

create table public.methods (
  id uuid not null default gen_random_uuid(), name text, description text, updated_at timestamptz not null default now(),
  constraint methods_pkey primary key (id));
create unique index methods_name_unique_ci on public.methods using btree (lower(name)) where (name is not null);

create table public.sermons (
  id uuid not null default gen_random_uuid(), title text not null, preacher text not null, date date not null,
  scripture text, sermon_resources_id uuid, summary text, content text not null, foc_id uuid not null, tags text[],
  profile_id uuid, updated_at timestamptz not null default now(), created_at timestamptz not null default now(),
  audio_url text, youtube_link text, facebook_link text, instagram_link text, twitter_link text, website_link text,
  constraint sermons_pkey primary key (id),
  constraint sermons_foc_id_fkey foreign key (foc_id) references public.foc_profile(id) on delete cascade,
  constraint sermons_profile_id_fkey foreign key (profile_id) references public.member_profiles(id) on update cascade on delete restrict);

create table public.events (
  id uuid not null default gen_random_uuid(), title text not null, date date not null,
  start_time timetz not null, end_time timetz not null, location text not null, description text, foc_id uuid,
  updated_at timestamptz, created_at timestamptz not null default now(), rsvp jsonb, "isRecurring" text,
  attendance jsonb default '{}'::jsonb, qr_code text, qr_generated_at timestamptz, sermon_id uuid,
  manual_stats jsonb default '{"arena": 0, "males": 0, "females": 0, "children": 0, "visitors": 0, "alterCall": 0, "salvation": 0, "reassurance": 0}'::jsonb,
  scope text not null default 'foc'::text, church_id uuid not null, province_id uuid,
  constraint events_pkey primary key (id),
  constraint events_church_id_fkey foreign key (church_id) references public.church(id) on update cascade on delete cascade,
  constraint events_foc_id_fkey foreign key (foc_id) references public.foc_profile(id) on update cascade on delete cascade,
  constraint events_province_id_fkey foreign key (province_id) references public.province(id) on update cascade on delete cascade,
  constraint events_sermon_id_fkey foreign key (sermon_id) references public.sermons(id) on update cascade on delete set null,
  constraint events_scope_check check (((scope = 'church'::text) and (province_id is null) and (foc_id is null)) or ((scope = 'province'::text) and (province_id is not null) and (foc_id is null)) or ((scope = 'foc'::text) and (foc_id is not null))),
  constraint events_scope_values check (scope = any (array['church'::text, 'province'::text, 'foc'::text])));

create table public.announcements (
  id uuid not null default gen_random_uuid(), title text not null, content text not null, foc_id uuid,
  priority text default 'medium'::text, category text, start_date date not null, end_date date,
  is_active bool default true, created_by uuid, created_at timestamptz not null default now(), updated_at timestamptz,
  scope text not null default 'foc'::text, church_id uuid not null, province_id uuid,
  constraint announcements_pkey primary key (id),
  constraint announcements_church_id_fkey foreign key (church_id) references public.church(id) on update cascade on delete cascade,
  constraint announcements_foc_id_fkey foreign key (foc_id) references public.foc_profile(id) on update cascade on delete cascade,
  constraint announcements_province_id_fkey foreign key (province_id) references public.province(id) on update cascade on delete cascade,
  constraint announcements_priority_check check (priority = any (array['low'::text, 'medium'::text, 'high'::text, 'urgent'::text])),
  constraint announcements_scope_check check (((scope = 'church'::text) and (province_id is null) and (foc_id is null)) or ((scope = 'province'::text) and (province_id is not null) and (foc_id is null)) or ((scope = 'foc'::text) and (foc_id is not null))),
  constraint announcements_scope_values check (scope = any (array['church'::text, 'province'::text, 'foc'::text])));

create table public.prayers (
  id uuid not null default gen_random_uuid(), title text not null, content text not null,
  is_anonymous bool not null default false, profile_id uuid not null, date date not null,
  created_at timestamptz not null default now(), support_count int4, foc_id uuid not null, updated_at timestamptz,
  reactions text[],
  constraint prayers_pkey primary key (id),
  constraint prayers_foc_id_fkey foreign key (foc_id) references public.foc_profile(id) on update cascade on delete cascade,
  constraint prayers_profile_id_fkey foreign key (profile_id) references public.member_profiles(id) on update cascade on delete cascade);

create table public.testimonies (
  id uuid not null default gen_random_uuid(), profile_id uuid, foc_id uuid not null, title varchar not null,
  situation text not null, intervention text not null, outcome text not null, is_anonymous bool default false,
  author varchar not null, status varchar default 'pending'::character varying, reactions jsonb default '[]'::jsonb,
  date date default current_date, created_at timestamptz default now(), updated_at timestamptz default now(),
  constraint testimonies_pkey primary key (id),
  constraint testimonies_foc_id_fkey foreign key (foc_id) references public.foc_profile(id) on delete cascade,
  constraint testimonies_identity_check check ((profile_id is not null) or (author is not null)),
  constraint testimonies_intervention_length check (char_length(intervention) <= 200),
  constraint testimonies_outcome_length check (char_length(outcome) <= 150),
  constraint testimonies_situation_length check (char_length(situation) <= 150),
  constraint testimonies_status_check check ((status)::text = any (array['pending'::text, 'approved'::text, 'rejected'::text])));

create table public.incomes (
  created_at timestamptz not null default now(), updated_at timestamptz default now(), member_id uuid,
  foc_id uuid not null, category_id uuid, amount float8 not null, currency_id uuid, method_id uuid, description text,
  date date not null default current_date, id uuid not null default gen_random_uuid(),
  constraint incomes_pkey primary key (id),
  constraint incomes_category_id_fkey foreign key (category_id) references public.categories(id) on update cascade on delete set null,
  constraint incomes_currency_id_fkey foreign key (currency_id) references public.currencies(id) on update cascade on delete set null,
  constraint incomes_foc_id_fkey foreign key (foc_id) references public.foc_profile(id) on update cascade on delete cascade,
  constraint incomes_member_id_fkey foreign key (member_id) references public.member_profiles(id) on update cascade on delete restrict,
  constraint incomes_method_id_fkey foreign key (method_id) references public.methods(id) on update cascade on delete set null);

create table public.expenses (
  created_at timestamptz not null default now(), updated_at timestamptz default now(), supplier text not null,
  foc_id uuid not null, ministry_id uuid, amount float8 not null, currency_id uuid, method_id uuid, description text,
  category_id uuid, date date not null default current_date, id uuid not null default gen_random_uuid(),
  constraint expenses_pkey primary key (id),
  constraint expenses_category_id_fkey foreign key (category_id) references public.categories(id) on update cascade on delete set null,
  constraint expenses_currency_id_fkey foreign key (currency_id) references public.currencies(id) on update cascade on delete set null,
  constraint expenses_foc_id_fkey foreign key (foc_id) references public.foc_profile(id) on update cascade on delete cascade,
  constraint expenses_method_id_fkey foreign key (method_id) references public.methods(id) on update cascade on delete set null,
  constraint expenses_ministry_id_fkey foreign key (ministry_id) references public.ministries(id) on update cascade on delete cascade);

create table public.contribution_changes (
  contribution_id int4 not null, contribution_type varchar not null, changed_by uuid, change_reason text not null,
  original_data jsonb not null, new_data jsonb not null, created_at timestamptz not null default now(),
  id uuid not null default gen_random_uuid(),
  constraint contribution_changes_pkey primary key (id),
  constraint contribution_changes_changed_by_fkey foreign key (changed_by) references auth.users(id) on update cascade on delete set null);

create table public.event_income (
  id uuid not null default gen_random_uuid(), event_id uuid not null, foc_id uuid not null, category_id uuid,
  currency_id uuid not null, amount numeric not null default 0, method text default 'cash'::text, notes text,
  recorded_by uuid, created_at timestamptz not null default now(), updated_at timestamptz default now(),
  constraint event_income_pkey primary key (id),
  constraint event_income_category_id_fkey foreign key (category_id) references public.categories(id) on delete set null,
  constraint event_income_currency_id_fkey foreign key (currency_id) references public.currencies(id) on delete restrict,
  constraint event_income_event_id_fkey foreign key (event_id) references public.events(id) on delete cascade,
  constraint event_income_foc_id_fkey foreign key (foc_id) references public.foc_profile(id) on delete cascade,
  constraint event_income_recorded_by_fkey foreign key (recorded_by) references public.member_profiles(id) on delete set null);

create table public.event_serving (
  id uuid not null default gen_random_uuid(), event_id uuid not null, profile_id uuid, ministry_id uuid, capacity text,
  marked_at timestamptz not null default now(), created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  constraint event_serving_pkey primary key (id),
  constraint event_serving_event_id_fkey foreign key (event_id) references public.events(id) on delete cascade,
  constraint event_serving_ministry_id_fkey foreign key (ministry_id) references public.ministries(id) on delete set null,
  constraint event_serving_profile_id_fkey foreign key (profile_id) references public.member_profiles(id) on update cascade on delete restrict,
  constraint event_serving_event_profile_unique unique (event_id, profile_id));

create table public.cell_meetings (
  id uuid not null default gen_random_uuid(), cell_group_id uuid not null, date date not null, title text, notes text,
  offering jsonb default '[]'::jsonb, attendance jsonb default '[]'::jsonb, created_by uuid,
  created_at timestamptz not null default now(), updated_at timestamptz default now(),
  constraint cell_meetings_pkey primary key (id),
  constraint cell_meetings_cell_group_id_fkey foreign key (cell_group_id) references public.cell_groups(id) on delete cascade,
  constraint cell_meetings_created_by_fkey foreign key (created_by) references public.member_profiles(id) on update cascade on delete restrict);

create table public.family_relationships (
  id uuid not null default gen_random_uuid(), member_id uuid not null, related_member_id uuid not null,
  relationship_type text not null, created_at timestamptz default now(),
  constraint family_relationships_pkey primary key (id),
  constraint fk_member foreign key (member_id) references public.member_profiles(id) on delete cascade,
  constraint fk_related_member foreign key (related_member_id) references public.member_profiles(id) on delete cascade,
  constraint no_self_relationships check (member_id <> related_member_id),
  constraint unique_relationship unique (member_id, related_member_id));

-- join tables: constraints for these were not in the dump (ASSUMED: cascade foreign keys)
create table public.foc_categories (
  id uuid not null default gen_random_uuid(), foc_id uuid not null, category_id uuid not null,
  created_at timestamptz not null default now(), constraint foc_categories_pkey primary key (id),
  constraint foc_categories_foc_fkey foreign key (foc_id) references public.foc_profile(id) on update cascade on delete cascade,
  constraint foc_categories_category_fkey foreign key (category_id) references public.categories(id) on update cascade on delete cascade);
create unique index foc_categories_unique on public.foc_categories using btree (foc_id, category_id);
create table public.foc_currencies (
  id uuid not null default gen_random_uuid(), foc_id uuid not null, currency_id uuid not null,
  created_at timestamptz not null default now(), constraint foc_currencies_pkey primary key (id),
  constraint foc_currencies_foc_fkey foreign key (foc_id) references public.foc_profile(id) on update cascade on delete cascade,
  constraint foc_currencies_currency_fkey foreign key (currency_id) references public.currencies(id) on update cascade on delete cascade);
create unique index foc_currencies_unique on public.foc_currencies using btree (foc_id, currency_id);
create table public.foc_methods (
  id uuid not null default gen_random_uuid(), foc_id uuid not null, method_id uuid not null,
  created_at timestamptz not null default now(), constraint foc_methods_pkey primary key (id),
  constraint foc_methods_foc_fkey foreign key (foc_id) references public.foc_profile(id) on update cascade on delete cascade,
  constraint foc_methods_method_fkey foreign key (method_id) references public.methods(id) on update cascade on delete cascade);
create unique index foc_methods_unique on public.foc_methods using btree (foc_id, method_id);
create table public.foc_ministries (
  foc_id uuid not null, ministry_id uuid not null, created_at timestamptz not null default now(),
  constraint foc_ministries_pkey primary key (foc_id, ministry_id),
  constraint foc_ministries_foc_id_fkey foreign key (foc_id) references public.foc_profile(id) on update cascade on delete cascade,
  constraint foc_ministries_ministry_id_fkey foreign key (ministry_id) references public.ministries(id) on update cascade on delete cascade);
create index foc_ministries_ministry_id_idx on public.foc_ministries using btree (ministry_id);

create table public.ministry_leaders (
  id uuid not null default gen_random_uuid(), ministry_id uuid not null, foc_id uuid not null,
  updated_at timestamptz default now(), leader_id uuid,
  constraint ministry_leaders_pkey primary key (id),
  constraint ministry_leaders_ministry_id_fkey foreign key (ministry_id) references public.ministries(id) on update cascade on delete cascade,
  constraint ministry_leaders_foc_id_fkey foreign key (foc_id) references public.foc_profile(id) on update cascade on delete cascade,
  constraint ministry_leaders_leader_id_fkey foreign key (leader_id) references public.member_profiles(id) on update cascade on delete set null);
create unique index ministry_leaders_ministry_foc_unique on public.ministry_leaders using btree (ministry_id, foc_id);

create table public.bible_reading_progress (
  id uuid not null default gen_random_uuid(), profile_id uuid not null, plan_id uuid not null,
  progress_data jsonb not null default '{}'::jsonb, created_at timestamptz default now(), updated_at timestamptz default now(),
  constraint bible_reading_progress_pkey primary key (id),
  constraint bible_reading_progress_plan_id_fkey foreign key (plan_id) references public.bible_reading_plans(id) on delete cascade,
  constraint bible_reading_progress_profile_id_fkey foreign key (profile_id) references public.member_profiles(id) on update cascade on delete cascade,
  constraint unique_profile_plan unique (profile_id, plan_id));
create index idx_bible_progress_plan_id on public.bible_reading_progress using btree (plan_id);
create index idx_bible_progress_profile_id on public.bible_reading_progress using btree (profile_id);
create index idx_bible_progress_data on public.bible_reading_progress using gin (progress_data);

create table public.social_events (
  id uuid not null default gen_random_uuid(), name varchar not null, type varchar not null, date date not null,
  description text, location varchar, time time, created_at timestamptz default current_timestamp,
  updated_at timestamptz default current_timestamp, created_by uuid, is_active bool default true, foc_id uuid not null,
  constraint social_events_pkey primary key (id),
  constraint social_events_created_by_fkey foreign key (created_by) references auth.users(id) on update cascade on delete cascade,
  constraint social_events_foc_id_fkey foreign key (foc_id) references public.foc_profile(id) on delete cascade,
  constraint social_events_type_check check ((type)::text = any ((array['Birthday'::character varying, 'Anniversary'::character varying, 'Memorial'::character varying, 'Other'::character varying])::text[])));

create table public.component_backgrounds (
  id uuid not null default uuid_generate_v4(), foc_id uuid, component_type text not null, image_url text not null,
  is_active bool default true, display_order int4 default 0, created_at timestamp default now(), updated_at timestamp default now(),
  constraint component_backgrounds_pkey primary key (id),
  constraint component_backgrounds_foc_id_fkey foreign key (foc_id) references public.foc_profile(id),
  constraint component_backgrounds_foc_id_component_type_display_order_key unique (foc_id, component_type, display_order));
create index idx_component_backgrounds_foc on public.component_backgrounds using btree (foc_id, component_type, is_active);

-- ---------------------------------- remaining indexes from the dump ----------------------------------
create index idx_announcements_foc_active on public.announcements using btree (foc_id, is_active, start_date);
create index idx_announcements_priority on public.announcements using btree (priority, start_date);
create index idx_announcements_scope_end on public.announcements using btree (scope, end_date);
create index idx_event_income_foc on public.event_income using btree (foc_id);
create index idx_event_income_event on public.event_income using btree (event_id);
create index idx_event_serving_event on public.event_serving using btree (event_id);
create index idx_event_serving_profile on public.event_serving using btree (profile_id);
create index idx_event_serving_ministry on public.event_serving using btree (ministry_id);
create index idx_family_member on public.family_relationships using btree (member_id);
create index idx_member_profiles_active on public.member_profiles using btree (foc_id) where (deleted_at is null);
create index idx_member_profiles_ministry_ids on public.member_profiles using gin (ministry_ids);
create index idx_testimonies_status on public.testimonies using btree (status);
create index idx_testimonies_created_at on public.testimonies using btree (created_at desc);
create index idx_testimonies_profile_id on public.testimonies using btree (profile_id);
create index idx_testimonies_foc_id on public.testimonies using btree (foc_id);
create index idx_testimonies_date on public.testimonies using btree (date desc);
-- ---------------------------------- functions (bodies copied from the dump) ----------------------------------
create function public.set_updated_at() returns trigger language plpgsql as $function$
begin new.updated_at = now(); return new; end; $function$;

create function public.update_updated_at_column() returns trigger language plpgsql as $function$
BEGIN NEW.updated_at = now(); RETURN NEW; END; $function$;

create function public.normalize_currency_name() returns trigger language plpgsql as $function$
begin new.name := upper(trim(new.name)); return new; end; $function$;

create function public.normalize_titlecase_name() returns trigger language plpgsql as $function$
begin new.name := initcap(trim(new.name)); return new; end; $function$;

create function public.prevent_category_type_change() returns trigger language plpgsql as $function$
begin
  if new.type is distinct from old.type
     and exists (select 1 from incomes  where category_id = old.id)
      or exists (select 1 from expenses where category_id = old.id) then
    raise exception 'Cannot change type of a category that is in use';
  end if;
  return new;
end $function$;

create function public.remove_course_from_profiles() returns trigger language plpgsql as $function$
begin
  update public.member_profiles set course_ids = array_remove(course_ids, old.id::text) where old.id::text = any(course_ids);
  return old;
end; $function$;

create function public.remove_ministry_from_profiles() returns trigger language plpgsql as $function$
begin
  update public.member_profiles set ministry_ids = array_remove(ministry_ids, old.id) where old.id = any(ministry_ids);
  return old;
end; $function$;

create function public.get_reciprocal_type(rtype text) returns text language plpgsql as $function$
BEGIN
    RETURN CASE
        WHEN rtype = 'parent' THEN 'child' WHEN rtype = 'child' THEN 'parent'
        WHEN rtype = 'grandparent' THEN 'grandchild' WHEN rtype = 'grandchild' THEN 'grandparent'
        WHEN rtype = 'spouse' THEN 'spouse' WHEN rtype = 'sibling' THEN 'sibling' ELSE 'relative' END;
END; $function$;

create function public.handle_family_reciprocity() returns trigger language plpgsql as $function$
DECLARE recip_type text;
BEGIN
    IF (TG_OP = 'INSERT') THEN
        recip_type := public.get_reciprocal_type(NEW.relationship_type);
        INSERT INTO public.family_relationships (member_id, related_member_id, relationship_type)
        VALUES (NEW.related_member_id, NEW.member_id, recip_type)
        ON CONFLICT (member_id, related_member_id) DO NOTHING;
        RETURN NEW;
    ELSIF (TG_OP = 'DELETE') THEN
        DELETE FROM public.family_relationships WHERE member_id = OLD.related_member_id AND related_member_id = OLD.member_id;
        RETURN OLD;
    END IF;
    RETURN NULL;
END; $function$;

-- broadcast triggers: note 'foc:' || NULL is NULL for church/province-scoped events and announcements
create function public.events_broadcast_trigger() returns trigger language plpgsql security definer as $function$
BEGIN
  PERFORM realtime.broadcast_changes('foc:' || COALESCE(NEW.foc_id, OLD.foc_id)::text, TG_OP, TG_OP, TG_TABLE_NAME, TG_TABLE_SCHEMA, NEW, OLD);
  RETURN COALESCE(NEW, OLD);
END; $function$;
create function public.announcements_broadcast_trigger() returns trigger language plpgsql security definer as $function$
BEGIN
  PERFORM realtime.broadcast_changes('foc:' || COALESCE(NEW.foc_id, OLD.foc_id)::text, TG_OP, TG_OP, TG_TABLE_NAME, TG_TABLE_SCHEMA, NEW, OLD);
  RETURN COALESCE(NEW, OLD);
END; $function$;
create function public.member_profiles_broadcast_trigger() returns trigger language plpgsql security definer as $function$
BEGIN
  PERFORM realtime.broadcast_changes('foc:' || COALESCE(NEW.foc_id, OLD.foc_id)::text, TG_OP, TG_OP, TG_TABLE_NAME, TG_TABLE_SCHEMA, NEW, OLD);
  RETURN COALESCE(NEW, OLD);
END; $function$;
create function public.prayers_broadcast_trigger() returns trigger language plpgsql security definer as $function$
BEGIN
  PERFORM realtime.broadcast_changes('foc:' || COALESCE(NEW.foc_id, OLD.foc_id)::text, TG_OP, TG_OP, TG_TABLE_NAME, TG_TABLE_SCHEMA, NEW, OLD);
  RETURN COALESCE(NEW, OLD);
END; $function$;
create function public.sermons_broadcast_trigger() returns trigger language plpgsql security definer as $function$
BEGIN
  PERFORM realtime.broadcast_changes('foc:' || COALESCE(NEW.foc_id, OLD.foc_id)::text, TG_OP, TG_OP, TG_TABLE_NAME, TG_TABLE_SCHEMA, NEW, OLD);
  RETURN COALESCE(NEW, OLD);
END; $function$;
create function public.testimonies_broadcast_trigger() returns trigger language plpgsql security definer as $function$
BEGIN
  PERFORM realtime.broadcast_changes('foc:' || COALESCE(NEW.foc_id, OLD.foc_id)::text, TG_OP, TG_OP, TG_TABLE_NAME, TG_TABLE_SCHEMA, NEW, OLD);
  RETURN COALESCE(NEW, OLD);
END; $function$;
create function public.cell_meetings_broadcast_trigger() returns trigger language plpgsql security definer set search_path to 'public' as $function$
declare v_church uuid;
begin
  select f.church_id into v_church from public.cell_groups g join public.foc_profile f on f.id = g.foc_id
   where g.id = coalesce(new.cell_group_id, old.cell_group_id);
  if v_church is null then return coalesce(new, old); end if;
  perform realtime.broadcast_changes('church:' || v_church::text, tg_op, tg_op, tg_table_name, tg_table_schema, new, old);
  return coalesce(new, old);
end $function$;

create function public.is_member_admin() returns boolean language sql stable security definer as $function$
  SELECT EXISTS (SELECT 1 FROM public.member_profiles WHERE user_id = auth.uid() AND role = 'admin');
$function$;

create function public.category_usage_count(p_foc_id uuid, p_category_id uuid) returns integer language sql stable as $function$
  select count(*)::int from (
    select id from public.incomes  where foc_id = p_foc_id and category_id = p_category_id
    union all
    select id from public.expenses where foc_id = p_foc_id and category_id = p_category_id) t;
$function$;
create function public.currency_usage_count(p_foc_id uuid, p_currency_id uuid) returns integer language sql stable as $function$
  select count(*)::int from (
    select id from public.incomes  where foc_id = p_foc_id and currency_id = p_currency_id
    union all
    select id from public.expenses where foc_id = p_foc_id and currency_id = p_currency_id) t;
$function$;
create function public.method_usage_count(p_foc_id uuid, p_method_id uuid) returns integer language sql stable as $function$
  select count(*)::int from (
    select id from public.incomes  where foc_id = p_foc_id and method_id = p_method_id
    union all
    select id from public.expenses where foc_id = p_foc_id and method_id = p_method_id) t;
$function$;

create function public.get_or_create_category(p_name text, p_type text, p_description text, p_foc_id uuid)
returns public.categories language plpgsql as $function$
declare v_row public.categories%rowtype; v_name text := initcap(trim(p_name));
begin
  insert into public.categories (name, type, description) values (v_name, p_type, p_description) on conflict ((lower(name))) do nothing;
  select * into v_row from public.categories where lower(name) = lower(v_name);
  insert into public.foc_categories (foc_id, category_id) values (p_foc_id, v_row.id) on conflict (foc_id, category_id) do nothing;
  return v_row;
end; $function$;
create function public.get_or_create_currency(p_name text, p_symbol text, p_rate double precision, p_foc_id uuid)
returns public.currencies language plpgsql as $function$
declare v_row public.currencies%rowtype; v_name text := upper(trim(p_name));
begin
  insert into public.currencies (name, symbol, rate) values (v_name, p_symbol, coalesce(p_rate, 1.00)) on conflict ((upper(name))) do nothing;
  select * into v_row from public.currencies where upper(name) = v_name;
  insert into public.foc_currencies (foc_id, currency_id) values (p_foc_id, v_row.id) on conflict (foc_id, currency_id) do nothing;
  return v_row;
end; $function$;
create function public.get_or_create_method(p_name text, p_description text, p_foc_id uuid)
returns public.methods language plpgsql as $function$
declare v_row public.methods%rowtype; v_name text := initcap(trim(p_name));
begin
  insert into public.methods (name, description) values (v_name, p_description) on conflict ((lower(name))) do nothing;
  select * into v_row from public.methods where lower(name) = lower(v_name);
  insert into public.foc_methods (foc_id, method_id) values (p_foc_id, v_row.id) on conflict (foc_id, method_id) do nothing;
  return v_row;
end; $function$;

create function public.get_visible_events(p_foc_id uuid, p_start date, p_end date) returns setof public.events language sql stable as $function$
  select e.* from events e join foc_profile fp on fp.id = p_foc_id
  where e.date >= p_start and e.date <= p_end
    and ((e.scope = 'church' and e.church_id = fp.church_id) or (e.scope = 'province' and e.province_id = fp.province_id) or (e.scope = 'foc' and e.foc_id = p_foc_id))
  order by e.date desc;
$function$;
create function public.get_visible_announcements(p_foc_id uuid) returns setof public.announcements language sql stable as $function$
  select a.* from public.announcements a join public.foc_profile f on f.id = p_foc_id
  where coalesce(a.is_active, true) and (a.end_date is null or a.end_date >= current_date - 1)
    and ((a.scope = 'foc' and a.foc_id = f.id) or (a.scope = 'province' and a.province_id = f.province_id) or (a.scope = 'church' and a.church_id = f.church_id));
$function$;
create function public.get_birthdays_today(p_foc_id uuid)
returns table(id uuid, first_name text, last_name text, profile_image text, dob date) language sql stable as $function$
  select id, first_name, last_name, profile_image, dob from member_profiles
  where foc_id = p_foc_id and dob is not null and extract(month from dob) = extract(month from current_date) and extract(day from dob) = extract(day from current_date);
$function$;
create function public.get_upcoming_birthdays(p_foc_id uuid, p_days integer default 7)
returns table(id uuid, first_name text, last_name text, profile_image text, dob date, days_away integer) language sql stable as $function$
  select id, first_name, last_name, profile_image, dob,
    (date_trunc('year', current_date)::date + (dob - date_trunc('year', dob)::date)
      + case when (dob - date_trunc('year', dob)::date) < (current_date - date_trunc('year', current_date)::date) then 365 else 0 end) - current_date as days_away
  from member_profiles where foc_id = p_foc_id and dob is not null
    and (date_trunc('year', current_date)::date + (dob - date_trunc('year', dob)::date)
      + case when (dob - date_trunc('year', dob)::date) < (current_date - date_trunc('year', current_date)::date) then 365 else 0 end) - current_date between 1 and p_days
  order by days_away;
$function$;
create function public.fn_member_profile_exists(phone_text text) returns boolean language sql stable as $function$
  SELECT EXISTS (SELECT 1 FROM public.member_profiles WHERE phone = phone_text LIMIT 1);
$function$;

-- registration RPCs (CONDENSED: same checks and the same INSERT; the disposable-domain list is shortened)
create function public.register_user_with_profile(user_id uuid, first_name text, last_name text, gender text, email text,
  phone text, foc_id uuid, terms_accepted_at timestamptz default null) returns jsonb
language plpgsql security definer set search_path to 'public' as $function$
DECLARE profile_record RECORD; clean_email TEXT; email_domain TEXT;
BEGIN
  clean_email := LOWER(TRIM(COALESCE(email, '')));
  IF clean_email = '' THEN RETURN jsonb_build_object('error', 'Email is required'); END IF;
  IF clean_email !~ '^[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,9}$' THEN RETURN jsonb_build_object('error', 'Invalid email address format'); END IF;
  email_domain := SPLIT_PART(clean_email, '@', 2);
  IF email_domain = ANY(ARRAY['mailinator.com','yopmail.com','tempmail.com','temp-mail.org','guerrillamail.com']) THEN
    RETURN jsonb_build_object('error', 'Please use a real email address. Disposable email services are not allowed'); END IF;
  IF EXISTS (SELECT 1 FROM member_profiles WHERE member_profiles.email = clean_email) THEN RETURN jsonb_build_object('error', 'An account with this email already exists'); END IF;
  IF TRIM(COALESCE(first_name, '')) = '' THEN RETURN jsonb_build_object('error', 'First name is required'); END IF;
  IF TRIM(COALESCE(last_name, '')) = '' THEN RETURN jsonb_build_object('error', 'Last name is required'); END IF;
  IF TRIM(COALESCE(phone, '')) = '' THEN RETURN jsonb_build_object('error', 'Phone number is required'); END IF;
  IF NOT EXISTS (SELECT 1 FROM foc_profile WHERE id = foc_id) THEN RETURN jsonb_build_object('error', 'Selected church does not exist'); END IF;
  IF terms_accepted_at IS NULL THEN RETURN jsonb_build_object('error', 'You must accept the Terms & Conditions'); END IF;
  INSERT INTO member_profiles (first_name, last_name, gender, email, phone, foc_id, user_id, role, status, terms_accepted_at, created_at, updated_at)
  VALUES (TRIM(first_name), TRIM(last_name), gender, clean_email, TRIM(phone), foc_id, user_id, 'member', 'pending', terms_accepted_at, NOW(), NOW())
  RETURNING * INTO profile_record;
  RETURN jsonb_build_object('success', true, 'profile_id', profile_record.id, 'user_id', user_id);
EXCEPTION WHEN OTHERS THEN RETURN jsonb_build_object('error', SQLERRM, 'detail', SQLSTATE);
END; $function$;

create function public.create_foc_and_user(church_data jsonb, user_data jsonb) returns jsonb
language plpgsql security definer as $function$
DECLARE new_foc_id UUID; new_profile_id UUID; church_uuid UUID; current_user_id UUID; result JSONB;
BEGIN
  current_user_id := auth.uid();
  IF current_user_id IS NULL THEN RAISE EXCEPTION 'User must be authenticated to create FOC and profile'; END IF;
  IF church_data IS NULL OR user_data IS NULL THEN RAISE EXCEPTION 'Both church_data and user_data are required'; END IF;
  IF NOT (church_data ? 'name' AND church_data ? 'address' AND church_data ? 'email' AND church_data ? 'contact' AND church_data ? 'church_id') THEN
    RAISE EXCEPTION 'Missing required church fields: name, address, email, contact, church_id'; END IF;
  IF NOT (user_data ? 'first_name' AND user_data ? 'last_name' AND user_data ? 'email' AND user_data ? 'phone' AND user_data ? 'gender') THEN
    RAISE EXCEPTION 'Missing required user fields: first_name, last_name, email, phone, gender'; END IF;
  IF EXISTS (SELECT 1 FROM member_profiles WHERE user_id = current_user_id) THEN RAISE EXCEPTION 'User profile already exists'; END IF;
  church_uuid := (church_data->>'church_id')::UUID;
  IF NOT EXISTS (SELECT 1 FROM church WHERE id = church_uuid) THEN RAISE EXCEPTION 'Church ID does not exist in church table'; END IF;
  INSERT INTO foc_profile (name, address, email, contact, country, church_id, logo)
  VALUES (church_data->>'name', church_data->>'address', church_data->>'email', church_data->>'contact',
          COALESCE(church_data->>'country', ''), church_uuid,
          COALESCE(church_data->>'logo', 'https://fcmeczxeixdczfmgdnra.supabase.co/storage/v1/object/public/churchpoint//churchpoint.png'))
  RETURNING id INTO new_foc_id;
  INSERT INTO member_profiles (user_id, first_name, last_name, email, phone, address, dob, gender, role, foc_id, created_at)
  VALUES (current_user_id, user_data->>'first_name', user_data->>'last_name', user_data->>'email', user_data->>'phone',
          COALESCE(user_data->>'address', ''),
          CASE WHEN user_data->>'dob' IS NOT NULL AND user_data->>'dob' != '' THEN (user_data->>'dob')::DATE ELSE NULL END,
          user_data->>'gender', COALESCE(user_data->>'role', 'admin'), new_foc_id, NOW())
  RETURNING id INTO new_profile_id;
  result := jsonb_build_object('success', true, 'data', jsonb_build_object('foc_id', new_foc_id, 'profile_id', new_profile_id, 'church_id', church_uuid, 'user_id', current_user_id));
  RETURN result;
EXCEPTION WHEN OTHERS THEN
  result := jsonb_build_object('success', false, 'error', SQLERRM, 'sqlstate', SQLSTATE);
  RETURN result;
END; $function$;
-- ---------------------------------- triggers (from the dump) ----------------------------------
create trigger events_broadcast after insert or delete or update on public.events for each row execute function public.events_broadcast_trigger();
create trigger ministry_deleted_cleanup after delete on public.ministries for each row execute function public.remove_ministry_from_profiles();
create trigger course_deleted_cleanup after delete on public.courses for each row execute function public.remove_course_from_profiles();
create trigger member_profiles_broadcast after insert or delete or update on public.member_profiles for each row execute function public.member_profiles_broadcast_trigger();
create trigger categories_normalize before insert or update on public.categories for each row execute function public.normalize_titlecase_name();
create trigger categories_type_lock before update of type on public.categories for each row execute function public.prevent_category_type_change();
create trigger currencies_normalize before insert or update on public.currencies for each row execute function public.normalize_currency_name();
create trigger methods_normalize before insert or update on public.methods for each row execute function public.normalize_titlecase_name();
create trigger prayers_broadcast after insert or delete or update on public.prayers for each row execute function public.prayers_broadcast_trigger();
create trigger sermons_broadcast after insert or delete or update on public.sermons for each row execute function public.sermons_broadcast_trigger();
create trigger update_bible_reading_progress_updated_at before update on public.bible_reading_progress for each row execute function public.update_updated_at_column();
create trigger announcements_broadcast after insert or delete or update on public.announcements for each row execute function public.announcements_broadcast_trigger();
create trigger trg_family_reciprocity after insert or delete on public.family_relationships for each row execute function public.handle_family_reciprocity();
create trigger trg_event_serving_updated_at before update on public.event_serving for each row execute function public.set_updated_at();
create trigger testimonies_broadcast after insert or delete or update on public.testimonies for each row execute function public.testimonies_broadcast_trigger();
create trigger update_testimonies_updated_at before update on public.testimonies for each row execute function public.update_updated_at_column();
create trigger cell_meetings_broadcast after insert or delete or update on public.cell_meetings for each row execute function public.cell_meetings_broadcast_trigger();
-- dead WhatsApp-OTP functions (STUBS with the production signatures; the originals reference a missing table)
create function public.send_whatsapp_otp(user_phone text) returns jsonb language plpgsql security definer as $$ begin return '{}'::jsonb; end $$;
create function public.verify_whatsapp_otp(user_phone text, user_otp text) returns jsonb language plpgsql security definer as $$ begin return '{}'::jsonb; end $$;
create function public.verify_whatsapp_otp(user_phone text, user_otp text, signup_data jsonb default null) returns jsonb language plpgsql security definer as $$ begin return '{}'::jsonb; end $$;
create function public.cleanup_expired_otps() returns integer language plpgsql security definer as $$ begin return 0; end $$;

-- ---------------------------------- Row Level Security: ON for these tables ----------------------------------
alter table public.announcements enable row level security;
alter table public.bible_reading_plans enable row level security;
alter table public.bible_reading_progress enable row level security;
alter table public.categories enable row level security;
alter table public.cell_groups enable row level security;
alter table public.church enable row level security;
alter table public.component_backgrounds enable row level security;
alter table public.contribution_changes enable row level security;
alter table public.courses enable row level security;
alter table public.currencies enable row level security;
alter table public.event_serving enable row level security;
alter table public.events enable row level security;
alter table public.expenses enable row level security;
alter table public.foc_profile enable row level security;
alter table public.incomes enable row level security;
alter table public.member_profiles enable row level security;
alter table public.methods enable row level security;
alter table public.ministries enable row level security;
alter table public.prayers enable row level security;
alter table public.sermons enable row level security;
alter table public.social_events enable row level security;
alter table public.testimonies enable row level security;
-- tables with RLS OFF in production (no policies): _sqlx_migrations, cell_meetings, event_income, family_relationships, foc_categories, foc_currencies, foc_methods, foc_ministries, ministry_leaders, province
-- ---------------------------------- policies (86, exact names from the dump) ----------------------------------
create policy "announcements_delete_admins_only" on public.announcements for delete to authenticated using (is_member_admin());
create policy "announcements_insert_admins_only" on public.announcements for insert to authenticated with check (is_member_admin());
create policy "announcements_select_for_authenticated" on public.announcements for select to authenticated using (true);
create policy "announcements_select_visible" on public.announcements for select to authenticated using ((EXISTS ( SELECT 1 FROM (member_profiles m JOIN foc_profile f ON ((f.id = m.foc_id))) WHERE ((m.user_id = auth.uid()) AND (((announcements.scope = 'foc'::text) AND (announcements.foc_id = f.id)) OR ((announcements.scope = 'province'::text) AND (announcements.province_id = f.province_id)) OR ((announcements.scope = 'church'::text) AND (announcements.church_id = f.church_id)))))));
create policy "announcements_update_admins_only" on public.announcements for update to authenticated using (is_member_admin()) with check (is_member_admin());
create policy "Allow authenticated users to create bible reading plans" on public.bible_reading_plans for insert to authenticated with check ((auth.uid() = user_id));
create policy "Allow authenticated users to view bible reading plans" on public.bible_reading_plans for select to authenticated using (true);
create policy "Allow creators to delete their own bible reading plans" on public.bible_reading_plans for delete to authenticated using ((auth.uid() = user_id));
create policy "Allow creators to update their own bible reading plans" on public.bible_reading_plans for update to authenticated using ((auth.uid() = user_id)) with check ((auth.uid() = user_id));
create policy "bible_reading_progress_delete_for_authenticated" on public.bible_reading_progress for delete to authenticated using (true);
create policy "bible_reading_progress_insert_for_authenticated" on public.bible_reading_progress for insert to authenticated with check (true);
create policy "bible_reading_progress_select_for_authenticated" on public.bible_reading_progress for select to authenticated using (true);
create policy "bible_reading_progress_update_for_authenticated" on public.bible_reading_progress for update to authenticated using (true) with check (true);
create policy "categories_delete_admins_only" on public.categories for delete to authenticated using (is_member_admin());
create policy "categories_insert_admins_only" on public.categories for insert to authenticated with check (is_member_admin());
create policy "categories_select_for_authenticated" on public.categories for select to authenticated using (true);
create policy "categories_update_admins_only" on public.categories for update to authenticated using (is_member_admin()) with check (is_member_admin());
create policy "cell_groups_delete_admins_only" on public.cell_groups for delete to authenticated using (is_member_admin());
create policy "cell_groups_insert_admins_only" on public.cell_groups for insert to authenticated with check (is_member_admin());
create policy "cell_groups_select_for_authenticated" on public.cell_groups for select to authenticated using (true);
create policy "cell_groups_update_admins_only" on public.cell_groups for update to authenticated using (is_member_admin()) with check (is_member_admin());
create policy "church_select_public" on public.church for select to public using (true);
create policy "component_backgrounds_delete_for_authenticated" on public.component_backgrounds for delete to authenticated using (true);
create policy "component_backgrounds_insert_for_authenticated" on public.component_backgrounds for insert to authenticated with check (true);
create policy "component_backgrounds_select_for_authenticated" on public.component_backgrounds for select to authenticated using (true);
create policy "component_backgrounds_update_for_authenticated" on public.component_backgrounds for update to authenticated using (true) with check (true);
create policy "contribution_changes_insert_admins_only" on public.contribution_changes for insert to authenticated with check (is_member_admin());
create policy "contribution_changes_select_admins_only" on public.contribution_changes for select to authenticated using (is_member_admin());
create policy "course_delete_admins_only" on public.courses for delete to authenticated using (is_member_admin());
create policy "course_insert_admins_only" on public.courses for insert to authenticated with check (is_member_admin());
create policy "course_select_for_authenticated" on public.courses for select to authenticated using (true);
create policy "course_update_admins_only" on public.courses for update to authenticated using (is_member_admin()) with check (is_member_admin());
create policy "currencies_delete_admins_only" on public.currencies for delete to authenticated using (is_member_admin());
create policy "currencies_insert_admins_only" on public.currencies for insert to authenticated with check (is_member_admin());
create policy "currencies_select_for_authenticated" on public.currencies for select to authenticated using (true);
create policy "currencies_update_admins_only" on public.currencies for update to authenticated using (is_member_admin()) with check (is_member_admin());
create policy "Members can update their own serving record" on public.event_serving for update to public using ((profile_id = ( SELECT member_profiles.id FROM member_profiles WHERE (member_profiles.user_id = auth.uid()) LIMIT 1)));
create policy "Members can upsert their own serving record" on public.event_serving for insert to public with check ((profile_id = ( SELECT member_profiles.id FROM member_profiles WHERE (member_profiles.user_id = auth.uid()) LIMIT 1)));
create policy "Members can view serving records for their FOC events" on public.event_serving for select to public using ((EXISTS ( SELECT 1 FROM (events e JOIN member_profiles mp ON (((mp.foc_id = e.foc_id) AND (mp.user_id = auth.uid())))) WHERE (e.id = event_serving.event_id))));
create policy "events_delete_admins_only" on public.events for delete to authenticated using (is_member_admin());
create policy "events_insert_admins_only" on public.events for insert to authenticated with check (is_member_admin());
create policy "events_select_for_authenticated" on public.events for select to authenticated using (true);
create policy "events_update_authenticated_only" on public.events for update to authenticated using (true) with check (true);
create policy "expenses_insert_authenticated_admins" on public.expenses for insert to authenticated with check (is_member_admin());
create policy "expenses_select_authenticated_admins" on public.expenses for select to authenticated using (is_member_admin());
create policy "expenses_update_authenticated_admins" on public.expenses for update to authenticated using (is_member_admin()) with check (is_member_admin());
create policy "foc_ministries delete" on public.foc_ministries for delete to authenticated using ((foc_id IN ( SELECT member_profiles.foc_id FROM member_profiles WHERE (member_profiles.id = auth.uid()))));
create policy "foc_ministries insert" on public.foc_ministries for insert to authenticated with check ((foc_id IN ( SELECT member_profiles.foc_id FROM member_profiles WHERE (member_profiles.id = auth.uid()))));
create policy "foc_ministries read" on public.foc_ministries for select to authenticated using (true);
create policy "foc_ministries update" on public.foc_ministries for update to authenticated using ((foc_id IN ( SELECT member_profiles.foc_id FROM member_profiles WHERE (member_profiles.id = auth.uid())))) with check ((foc_id IN ( SELECT member_profiles.foc_id FROM member_profiles WHERE (member_profiles.id = auth.uid()))));
create policy "foc_profile_delete_admins_only" on public.foc_profile for delete to authenticated using (is_member_admin());
create policy "foc_profile_insert_admins_only" on public.foc_profile for insert to authenticated with check (is_member_admin());
create policy "foc_profile_select_for_authenticated" on public.foc_profile for select to public using (true);
create policy "foc_profile_update_admins_only" on public.foc_profile for update to authenticated using (is_member_admin()) with check (is_member_admin());
create policy "incomes_insert_authenticated_admins" on public.incomes for insert to authenticated with check (is_member_admin());
create policy "incomes_select_authenticated" on public.incomes for select to authenticated using (true);
create policy "incomes_update_authenticated_admins" on public.incomes for update to authenticated using (is_member_admin()) with check (is_member_admin());
create policy "member_profiles_insert_authenticated" on public.member_profiles for insert to authenticated with check (true);
create policy "member_profiles_select_authenticated" on public.member_profiles for select to public using (true);
create policy "member_profiles_update_authenticated" on public.member_profiles for update to authenticated using (true) with check (true);
create policy "methods_delete_admins_only" on public.methods for delete to authenticated using (is_member_admin());
create policy "methods_insert_admins_only" on public.methods for insert to authenticated with check (is_member_admin());
create policy "methods_select_for_authenticated" on public.methods for select to authenticated using (true);
create policy "methods_update_admins_only" on public.methods for update to authenticated using (is_member_admin()) with check (is_member_admin());
create policy "ministries read all" on public.ministries for select to authenticated using (true);
create policy "ministries_delete_admins_only" on public.ministries for delete to authenticated using (is_member_admin());
create policy "ministries_insert_admins_only" on public.ministries for insert to authenticated with check (is_member_admin());
create policy "ministries_select_for_authenticated" on public.ministries for select to authenticated using (true);
create policy "ministries_update_admins_only" on public.ministries for update to authenticated using (is_member_admin()) with check (is_member_admin());
create policy "prayers_delete_authenticated" on public.prayers for delete to authenticated using (true);
create policy "prayers_insert_authenticated" on public.prayers for insert to authenticated with check (true);
create policy "prayers_select_authenticated" on public.prayers for select to authenticated using (true);
create policy "prayers_update_authenticated" on public.prayers for update to authenticated using (true) with check (true);
create policy "sermons_delete_admins_only" on public.sermons for delete to authenticated using (is_member_admin());
create policy "sermons_insert_admins_only" on public.sermons for insert to authenticated with check (is_member_admin());
create policy "sermons_select_for_authenticated" on public.sermons for select to authenticated using (true);
create policy "sermons_update_admins_only" on public.sermons for update to authenticated using (is_member_admin()) with check (is_member_admin());
create policy "social_events_delete_authenticated" on public.social_events for delete to authenticated using (true);
create policy "social_events_insert_authenticated" on public.social_events for insert to authenticated with check (true);
create policy "social_events_select_authenticated" on public.social_events for select to authenticated using (true);
create policy "social_events_update_authenticated" on public.social_events for update to authenticated using (true) with check (true);
create policy "delete_own_testimonies" on public.testimonies for delete to public using ((((auth.uid() = profile_id) AND ((status)::text = 'pending'::text)) OR (EXISTS ( SELECT 1 FROM member_profiles mp WHERE ((mp.user_id = auth.uid()) AND (mp.role = 'admin'::text))))));
create policy "insert_own_testimonies" on public.testimonies for insert to public with check ((auth.uid() = profile_id));
create policy "update_own_testimonies" on public.testimonies for update to public using ((((auth.uid() = profile_id) AND ((status)::text = 'pending'::text)) OR (EXISTS ( SELECT 1 FROM member_profiles mp WHERE ((mp.user_id = auth.uid()) AND (mp.role = 'admin'::text))))));
create policy "view_approved_testimonies" on public.testimonies for select to public using ((((status)::text = 'approved'::text) AND (EXISTS ( SELECT 1 FROM member_profiles mp WHERE ((mp.user_id = auth.uid()) AND (mp.foc_id = testimonies.foc_id))))));
create policy "view_own_testimonies" on public.testimonies for select to public using ((auth.uid() = profile_id));

-- ---------------------------------- privileges: Supabase grants API roles access to every table; RLS decides rows ----------------------------------
grant usage on schema public to anon, authenticated, service_role;
grant all on all tables in schema public to anon, authenticated, service_role;
grant all on all sequences in schema public to anon, authenticated, service_role;
grant all on all functions in schema public to anon, authenticated, service_role;
alter default privileges in schema public grant all on tables to anon, authenticated, service_role;
alter default privileges in schema public grant all on sequences to anon, authenticated, service_role;
