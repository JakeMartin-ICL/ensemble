create index if not exists weave_sessions_active_host_created_idx
  on public.weave_sessions(host_user_id, created_at desc)
  where is_active = true;
