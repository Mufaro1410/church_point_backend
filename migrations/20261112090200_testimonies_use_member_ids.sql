-- Phase 2c: testimonies.profile_id held the AUTH user id while every other table (and the Axum API) uses the
-- MEMBER id. Convert the data, add the missing foreign key, and rewrite the row-level policies to match.
-- The mobile app must send the member id too (see the notes that came with this patch).

create or replace function public.current_member_id() returns uuid
language sql stable security definer set search_path = public as $$
  select id from public.member_profiles where user_id = auth.uid() and deleted_at is null limit 1
$$;

-- auth user id -> member id
update public.testimonies t
   set profile_id = m.id
  from public.member_profiles m
 where t.profile_id = m.user_id and t.profile_id <> m.id;

-- anything that is still not a member id has no owner we can identify (the author name stays on the row)
update public.testimonies t set profile_id = null
 where profile_id is not null
   and not exists (select 1 from public.member_profiles m where m.id = t.profile_id);

alter table public.testimonies drop constraint if exists testimonies_profile_id_fkey;
alter table public.testimonies add constraint testimonies_profile_id_fkey
  foreign key (profile_id) references public.member_profiles (id) on update cascade on delete set null;

drop policy if exists insert_own_testimonies on public.testimonies;
drop policy if exists view_own_testimonies   on public.testimonies;
drop policy if exists update_own_testimonies on public.testimonies;
drop policy if exists delete_own_testimonies on public.testimonies;

create policy insert_own_testimonies on public.testimonies for insert to public
  with check (profile_id = public.current_member_id());
create policy view_own_testimonies on public.testimonies for select to public
  using (profile_id = public.current_member_id());
create policy update_own_testimonies on public.testimonies for update to public
  using ((profile_id = public.current_member_id() and status::text = 'pending') or public.is_member_admin());
create policy delete_own_testimonies on public.testimonies for delete to public
  using ((profile_id = public.current_member_id() and status::text = 'pending') or public.is_member_admin());

do $$
begin
  if exists (select 1 from public.testimonies t where profile_id is not null
             and not exists (select 1 from public.member_profiles m where m.id = t.profile_id)) then
    raise exception 'testimonies still reference ids that are not members';
  end if;
end $$;
