alter table public.party_sessions
  drop constraint if exists party_sessions_mode_check;

alter table public.party_sessions
  add constraint party_sessions_mode_check
  check (mode in ('open_queue', 'shared_queue'));
