alter table public.party_sessions
  add column playback_track_uri    text,
  add column playback_progress_ms  bigint,
  add column playback_duration_ms  bigint,
  add column playback_is_playing   boolean,
  add column playback_updated_at   timestamptz;
