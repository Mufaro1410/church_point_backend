-- Phase 2b: remove the WhatsApp-OTP leftovers (login is email + password now) and fix a trigger bug.
drop function if exists public.send_whatsapp_otp(text);
drop function if exists public.verify_whatsapp_otp(text, text);
drop function if exists public.verify_whatsapp_otp(text, text, jsonb);
drop function if exists public.cleanup_expired_otps();
drop function if exists public.fn_member_profile_exists(text);

-- Bug: `A and B or C` meant that ANY category used by an expense blocked every `update of type`, even a no-op.
create or replace function public.prevent_category_type_change() returns trigger language plpgsql as $function$
begin
  if new.type is distinct from old.type
     and (exists (select 1 from public.incomes  where category_id = old.id)
          or exists (select 1 from public.expenses where category_id = old.id)) then
    raise exception 'Cannot change type of a category that is in use';
  end if;
  return new;
end $function$;

do $$
begin
  if exists (select 1 from pg_proc where pronamespace = 'public'::regnamespace
             and proname in ('send_whatsapp_otp', 'verify_whatsapp_otp', 'cleanup_expired_otps', 'fn_member_profile_exists')) then
    raise exception 'the OTP functions are still present';
  end if;
end $$;
