-- A member's email is their login identity. Direct API clients (the Supabase anon/authenticated roles)
-- can never change it; only the backend's privileged role (admin tools) can.
CREATE OR REPLACE FUNCTION public.guard_member_email() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.email IS DISTINCT FROM OLD.email AND current_user IN ('anon', 'authenticated') THEN
    RAISE EXCEPTION 'Email address cannot be changed' USING ERRCODE = '42501';
  END IF;
  RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS member_profiles_guard_email ON public.member_profiles;
CREATE TRIGGER member_profiles_guard_email
  BEFORE UPDATE OF email ON public.member_profiles
  FOR EACH ROW EXECUTE FUNCTION public.guard_member_email();
