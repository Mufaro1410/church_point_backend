-- Hand counts are one fixed list for every church again, kept in events.manual_stats:
-- males, females, children, arena (youth), visitors, salvation, alterCall, reassurance.
-- Anything entered through the short-lived counters tables is copied back, then those tables go.

do $$
begin
  if to_regclass('public.event_counts') is not null then
    update public.events e set manual_stats = coalesce(e.manual_stats, '{}'::jsonb)
      - 'alter_call' - 'rededication' - 'age_children' - 'teens' - 'youth' - 'adults' - 'seniors' - 'main_hall' - 'balcony' - 'overflow_hall'
      - 'outside' - 'childrens_church' - 'total_attendance' - 'returning_visitors' - 'special_guests' - 'water_baptism' - 'spirit_baptism'
      - 'healing_testimonies' - 'deliverance' - 'prayer_line' - 'ushers' - 'choir' - 'media_team' - 'security' - 'sunday_school_teachers'
      - 'nursery' - 'volunteers' - 'livestream_viewers' - 'online_peak' - 'new_members' - 'communion' - 'ministers' - 'vehicles'
      || coalesce((
        select jsonb_object_agg(
                 case c.key when 'alter_call' then 'alterCall' when 'rededication' then 'reassurance' else c.key end, x.value)
        from public.event_counts x join public.counter_catalogue c on c.id = x.counter_id
        where x.event_id = e.id
          and c.key in ('males', 'females', 'children', 'arena', 'visitors', 'salvation', 'alter_call', 'rededication')), '{}'::jsonb)
    where exists (select 1 from public.event_counts x where x.event_id = e.id);
  end if;
end $$;

drop trigger if exists event_counts_sync on public.event_counts;
drop trigger if exists foc_default_counters on public.foc_profile;
drop function if exists public.sync_event_manual_stats();
drop function if exists public.give_new_foc_default_counters();
drop table if exists public.event_counts;
drop table if exists public.foc_counters;
drop table if exists public.counter_catalogue;
