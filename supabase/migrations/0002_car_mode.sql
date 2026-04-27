create type public.car_turn as enum ('a', 'b');

create table public.car_sessions (
  id                  uuid        primary key default gen_random_uuid(),
  host_user_id        uuid        not null references public.users(id),
  playlist_a_id       text        not null,
  playlist_b_id       text        not null,
  playlist_a_name     text        not null,
  playlist_b_name     text        not null,
  current_turn        car_turn    not null default 'a',
  playlist_a_order    text[]      not null default '{}',
  playlist_b_order    text[]      not null default '{}',
  playlist_a_index    integer     not null default 0,
  playlist_b_index    integer     not null default 0,
  current_track_uri   text,
  queued_track_uri    text,
  is_active           boolean     not null default true,
  created_at          timestamptz not null default now(),
  updated_at          timestamptz not null default now()
);

alter table public.car_sessions enable row level security;

create policy "Host can read own sessions"
  on public.car_sessions for select
  using (host_user_id = (
    select id from public.users
    where spotify_id = current_setting('app.spotify_id', true)
  ));

create policy "Host can update own sessions"
  on public.car_sessions for update
  using (host_user_id = (
    select id from public.users
    where spotify_id = current_setting('app.spotify_id', true)
  ));
