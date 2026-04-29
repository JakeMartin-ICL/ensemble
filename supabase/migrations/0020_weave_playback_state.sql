alter table public.weave_sessions
  add column if not exists playback_track_uri    text,
  add column if not exists playback_progress_ms  bigint,
  add column if not exists playback_duration_ms  bigint,
  add column if not exists playback_is_playing   boolean,
  add column if not exists playback_updated_at   timestamptz;
