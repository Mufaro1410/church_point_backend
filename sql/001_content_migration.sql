-- 001_content_migration.sql
-- Run this ONCE in the Supabase SQL editor BEFORE deploying the content patch.
-- It only ADDS nullable columns, so the existing React app keeps working unchanged.

begin;

-- 1. Who created a church-wide / province-wide row? (those rows have foc_id = NULL, so today
--    there is no way to know). Rows with NULL here stay editable by any admin who can see
--    them (the previous behaviour); new rows record their creating FOC.
alter table public.events
  add column if not exists created_by_foc_id uuid
  references public.foc_profile (id) on update cascade on delete set null;

alter table public.announcements
  add column if not exists created_by_foc_id uuid
  references public.foc_profile (id) on update cascade on delete set null;

-- Best-effort backfill for announcements: created_by holds the creator's auth user id
-- (or, in some rows, a member id). Events have no creator column, so they stay NULL.
update public.announcements a
   set created_by_foc_id = m.foc_id
  from public.member_profiles m
 where a.created_by_foc_id is null
   and a.scope in ('church', 'province')
   and (m.user_id = a.created_by or m.id = a.created_by)
   and m.foc_id is not null;

-- 2. Moderation audit trail for testimonies.
alter table public.testimonies
  add column if not exists moderated_by uuid
    references public.member_profiles (id) on update cascade on delete set null,
  add column if not exists moderated_at timestamptz;

commit;
