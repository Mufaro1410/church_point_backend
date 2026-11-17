-- FOCs must be approved by the platform owner (super_admin) before they can use the API.
-- Existing FOCs stay approved; new ones are created as 'pending' by the app.
alter table public.foc_profile add column if not exists status text not null default 'approved';
alter table public.foc_profile drop constraint if exists foc_profile_status_check;
alter table public.foc_profile add constraint foc_profile_status_check
  check (status in ('pending','approved','suspended'));
create index if not exists foc_profile_status_idx on public.foc_profile(status);

do $$ begin
  if not exists (select 1 from information_schema.columns
                 where table_schema='public' and table_name='foc_profile' and column_name='status') then
    raise exception 'foc_profile.status missing';
  end if;
end $$;
