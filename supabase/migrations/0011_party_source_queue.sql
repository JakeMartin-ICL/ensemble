alter table public.party_sessions
  add column source_min_queue_size integer not null default 0,
  add column add_added_tracks_to_source boolean not null default false,
  add constraint party_sessions_source_min_queue_size_check check (source_min_queue_size >= 0 and source_min_queue_size <= 25);

create table public.party_source_queue_items (
  id                  uuid        primary key default gen_random_uuid(),
  session_id          uuid        not null references public.party_sessions(id) on delete cascade,
  position            integer     not null,
  track               jsonb       not null,
  added_by_user_id    uuid        references public.users(id),
  created_at          timestamptz not null default now()
);

create index party_source_queue_items_session_position_idx
  on public.party_source_queue_items (session_id, position);

create unique index party_source_queue_items_session_track_uri_idx
  on public.party_source_queue_items (session_id, (track->>'uri'));

alter table public.party_source_queue_items enable row level security;

create policy "Host can read own party source queue"
  on public.party_source_queue_items for select
  using (exists (
    select 1 from public.party_sessions s
    where s.id = session_id
      and s.host_user_id = (
        select id from public.users
        where spotify_id = current_setting('app.spotify_id', true)
      )
  ));
