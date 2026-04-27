alter table public.car_sessions
  rename to weave_sessions;

alter type public.car_turn
  rename to weave_turn;

alter index if exists public.car_sessions_pkey
  rename to weave_sessions_pkey;

alter policy "Host can read own car sessions"
  on public.weave_sessions
  rename to "Host can read own weave sessions";

alter policy "Host can update own car sessions"
  on public.weave_sessions
  rename to "Host can update own weave sessions";
