-- When a church last changed a currency's exchange rate (shown on Home and in Finance settings).
alter table public.foc_currencies add column if not exists rate_updated_at timestamptz;
