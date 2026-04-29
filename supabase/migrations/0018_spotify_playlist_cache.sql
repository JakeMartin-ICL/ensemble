create table public.spotify_user_playlist_cache (
  user_id     uuid        primary key references public.users(id) on delete cascade,
  playlists   jsonb       not null,
  fetched_at  timestamptz not null default now(),
  expires_at  timestamptz not null
);

create table public.spotify_playlist_track_cache (
  playlist_id text        primary key,
  name        text        not null,
  snapshot_id text,
  track_count integer     not null,
  tracks      jsonb       not null,
  fetched_at  timestamptz not null default now(),
  expires_at  timestamptz not null
);

create index spotify_user_playlist_cache_expires_at_idx
  on public.spotify_user_playlist_cache (expires_at);

create index spotify_playlist_track_cache_expires_at_idx
  on public.spotify_playlist_track_cache (expires_at);
