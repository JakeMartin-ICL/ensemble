alter table public.party_sessions
  drop constraint if exists party_sessions_mode_check;

alter table public.party_sessions
  add constraint party_sessions_mode_check
  check (mode in ('open_queue', 'shared_queue', 'voted_queue'));

alter table public.party_queue_items
  add column pin_position integer;

create table public.party_queue_votes (
  id             uuid        primary key default gen_random_uuid(),
  queue_item_id  uuid        not null references public.party_queue_items(id) on delete cascade,
  user_id        uuid        not null references public.users(id) on delete cascade,
  created_at     timestamptz not null default now(),
  unique(queue_item_id, user_id)
);

alter table public.party_queue_votes enable row level security;

create policy "Host can read own party queue votes"
  on public.party_queue_votes for select
  using (exists (
    select 1 from public.party_queue_items qi
    join public.party_sessions s on s.id = qi.session_id
    where qi.id = queue_item_id
      and s.host_user_id = (
        select id from public.users
        where spotify_id = current_setting('app.spotify_id', true)
      )
  ));
