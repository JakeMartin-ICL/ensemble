alter table public.users
  add column spotify_client_id text;

create table public.party_guests (
  id           uuid        primary key default gen_random_uuid(),
  session_id   uuid        not null references public.party_sessions(id) on delete cascade,
  display_name text        not null,
  token_hash   text        not null unique,
  expires_at   timestamptz not null,
  created_at   timestamptz not null default now(),
  updated_at   timestamptz not null default now()
);

create index party_guests_session_id_idx
  on public.party_guests(session_id);

create index party_guests_expires_at_idx
  on public.party_guests(expires_at);

alter table public.party_queue_items
  add column added_by_guest_id uuid references public.party_guests(id) on delete set null;

alter table public.party_source_queue_items
  add column added_by_guest_id uuid references public.party_guests(id) on delete set null;

alter table public.party_played_tracks
  add column added_by_guest_id uuid references public.party_guests(id) on delete set null;

alter table public.party_queue_votes
  alter column user_id drop not null,
  add column guest_id uuid references public.party_guests(id) on delete cascade,
  add constraint party_queue_votes_one_actor_check check (
    (user_id is not null and guest_id is null)
    or (user_id is null and guest_id is not null)
  );

alter table public.party_queue_votes
  drop constraint if exists party_queue_votes_queue_item_id_user_id_key;

create unique index party_queue_votes_queue_user_idx
  on public.party_queue_votes(queue_item_id, user_id)
  where user_id is not null;

create unique index party_queue_votes_queue_guest_idx
  on public.party_queue_votes(queue_item_id, guest_id)
  where guest_id is not null;

alter table public.party_guests enable row level security;
