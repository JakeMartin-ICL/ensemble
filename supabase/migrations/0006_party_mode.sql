create table public.party_sessions (
  id                  uuid        primary key default gen_random_uuid(),
  host_user_id        uuid        not null references public.users(id),
  room_code           text        not null unique,
  mode                text        not null default 'open_queue',
  current_track_uri   text,
  queued_track_uri    text,
  is_active           boolean     not null default true,
  created_at          timestamptz not null default now(),
  updated_at          timestamptz not null default now(),
  constraint party_sessions_mode_check check (mode in ('open_queue'))
);

create table public.party_queue_items (
  id                  uuid        primary key default gen_random_uuid(),
  session_id          uuid        not null references public.party_sessions(id) on delete cascade,
  position            integer     not null,
  track               jsonb       not null,
  added_by_user_id    uuid        references public.users(id),
  created_at          timestamptz not null default now()
);

create index party_queue_items_session_position_idx
  on public.party_queue_items (session_id, position);

alter table public.party_sessions enable row level security;
alter table public.party_queue_items enable row level security;

create policy "Host can read own party sessions"
  on public.party_sessions for select
  using (host_user_id = (
    select id from public.users
    where spotify_id = current_setting('app.spotify_id', true)
  ));

create policy "Host can update own party sessions"
  on public.party_sessions for update
  using (host_user_id = (
    select id from public.users
    where spotify_id = current_setting('app.spotify_id', true)
  ));

create policy "Host can read own party queue"
  on public.party_queue_items for select
  using (exists (
    select 1 from public.party_sessions s
    where s.id = session_id
      and s.host_user_id = (
        select id from public.users
        where spotify_id = current_setting('app.spotify_id', true)
      )
  ));
