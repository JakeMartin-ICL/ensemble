create table public.party_played_tracks (
  id                       uuid        primary key default gen_random_uuid(),
  session_id               uuid        not null references public.party_sessions(id) on delete cascade,
  play_order               integer     not null,
  track                    jsonb       not null,
  added_by_user_id         uuid        references public.users(id),
  created_at               timestamptz not null default now()
);

create index party_played_tracks_session_play_order_idx
  on public.party_played_tracks (session_id, play_order);

alter table public.party_played_tracks enable row level security;

create policy "Host can read own party played tracks"
  on public.party_played_tracks for select
  using (exists (
    select 1 from public.party_sessions s
    where s.id = session_id
      and s.host_user_id = (
        select id from public.users
        where spotify_id = current_setting('app.spotify_id', true)
      )
  ));
