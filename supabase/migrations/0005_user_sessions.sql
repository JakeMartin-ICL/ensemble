create extension if not exists pgcrypto;

create table public.user_sessions (
  id           uuid        primary key default gen_random_uuid(),
  user_id      uuid        not null references public.users(id) on delete cascade,
  token_hash   text        not null unique,
  expires_at   timestamptz not null,
  created_at   timestamptz not null default now()
);

create index user_sessions_user_id_idx on public.user_sessions(user_id);
create index user_sessions_expires_at_idx on public.user_sessions(expires_at);

alter table public.user_sessions enable row level security;
