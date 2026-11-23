-- Two built-in roles only: admin (everything) and member. Every other role is a custom role the church creates
-- (church_roles). The old fixed "cell_leader" role never carried any permissions, so those people become members;
-- the church can create a "Cell leader" role and give it to them.

update public.member_profiles set role = 'member' where role = 'cell_leader';

alter table public.member_profiles drop constraint if exists member_profiles_role_check;
alter table public.member_profiles add constraint member_profiles_role_check
  check (role in ('member', 'admin', 'super_admin'));
