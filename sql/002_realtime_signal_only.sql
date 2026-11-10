-- 002_realtime_signal_only.sql   (PROPOSAL: read, adapt and test on a COPY of your project first)
--
-- Problem: if your broadcast triggers send the whole row, then every subscriber receives
-- prayers/testimonies WITH their author fields, which defeats the anonymity that the Axum
-- API now enforces. Rows of a single FOC are also sent on the church-wide channel.
--
-- Fix: broadcast only a tiny SIGNAL ({table, id, action}) on a PRIVATE per-FOC topic.
-- Clients react by refetching through Axum, which applies tenant and anonymity rules again.
--
-- I could not run this against Supabase Realtime itself: I verified the trigger logic against a
-- stub of realtime.send(). Confirm the realtime.send signature in your project and test it.

-- Step 0: find the broadcast triggers you have today, then drop the ones on these tables.
--   select tgname, tgrelid::regclass from pg_trigger
--    where not tgisinternal and tgrelid::regclass::text in ('prayers','testimonies','sermons');
--   -- e.g.  drop trigger prayers_broadcast on public.prayers;

create or replace function public.content_signal() returns trigger
language plpgsql security definer set search_path = '' as $$
declare r record;
begin
  r := case when tg_op = 'DELETE' then old else new end;
  -- No row data, no author, no content: just "something changed here".
  perform realtime.send(
    jsonb_build_object('table', tg_table_name, 'id', r.id, 'action', tg_op),
    tg_op,
    'foc:' || r.foc_id::text,
    true            -- private channel
  );
  return null;
end $$;

create trigger prayers_signal      after insert or update or delete on public.prayers      for each row execute function public.content_signal();
create trigger testimonies_signal  after insert or update or delete on public.testimonies  for each row execute function public.content_signal();
create trigger sermons_signal      after insert or update or delete on public.sermons      for each row execute function public.content_signal();

-- Private channels need an RLS policy on realtime.messages, otherwise nobody may subscribe.
-- Template (adapt, and verify realtime.topic() behaviour in your project):
--
-- create policy "members listen to their own FOC" on realtime.messages
--   for select to authenticated
--   using (realtime.topic() = 'foc:' || (
--     select foc_id::text from public.member_profiles
--      where user_id = auth.uid() and deleted_at is null limit 1));
--
-- Client change: subscribe to  supabase.channel('foc:' + focId, { config: { private: true } })
-- and on any signal call the matching Axum list endpoint instead of merging the payload.
