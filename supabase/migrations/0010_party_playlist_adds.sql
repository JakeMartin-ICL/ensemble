alter table public.party_sessions
  add column allow_guest_playlist_adds boolean not null default false;
