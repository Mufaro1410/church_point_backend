-- Remembers the answer to a write request, so sending the same request twice (double click,
-- flaky network retry) does the work once and returns the same answer.
create table if not exists idempotency_keys (
    user_id     uuid        not null,
    key         text        not null,
    method      text        not null,
    path        text        not null,
    status_code integer,            -- null while the first request is still running
    response    jsonb,
    created_at  timestamptz not null default now(),
    primary key (user_id, key)
);

create index if not exists idempotency_keys_created_idx on idempotency_keys (created_at);

alter table idempotency_keys enable row level security;
revoke all on idempotency_keys from anon, authenticated;
