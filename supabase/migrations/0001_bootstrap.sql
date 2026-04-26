create table public.users (
  id                uuid        primary key default gen_random_uuid(),
  spotify_id        text        not null unique,
  display_name      text        not null,
  access_token      text        not null,
  refresh_token     text        not null,
  token_expires_at  timestamptz not null,
  created_at        timestamptz not null default now(),
  updated_at        timestamptz not null default now()
);

alter table public.users enable row level security;

create policy "Users can read own row"
  on public.users for select
  using (spotify_id = current_setting('app.spotify_id', true));

create policy "Users can update own row"
  on public.users for update
  using (spotify_id = current_setting('app.spotify_id', true));
