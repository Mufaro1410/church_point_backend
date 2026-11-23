-- Anonymous posting is replaced by "share with church leaders only".
-- `is_anonymous` on prayers and testimonies now means: not shown in the feed; visible to the author and to church
-- leaders (admins and anyone whose role can view prayers or testimonies). Posts made anonymously before this change
-- become leaders-only automatically, so nobody's expectation of privacy is broken.

-- Members under 13 need a parent or guardian to agree.
alter table public.member_profiles
    add column if not exists guardian_name text,
    add column if not exists guardian_consent_at timestamptz;
