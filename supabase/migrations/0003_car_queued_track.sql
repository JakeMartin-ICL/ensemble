alter table public.car_sessions
  add column if not exists queued_track_uri text;
