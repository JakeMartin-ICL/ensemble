alter table public.car_sessions
  add column if not exists playlists jsonb not null default '[]'::jsonb,
  add column if not exists current_playlist_index integer not null default 0,
  add column if not exists playlist_track_indexes integer[] not null default '{}';

update public.car_sessions
set
  playlists = jsonb_build_array(
    jsonb_build_object(
      'id', playlist_a_id,
      'name', playlist_a_name,
      'order', to_jsonb(playlist_a_order)
    ),
    jsonb_build_object(
      'id', playlist_b_id,
      'name', playlist_b_name,
      'order', to_jsonb(playlist_b_order)
    )
  ),
  current_playlist_index = case current_turn when 'a'::car_turn then 0 else 1 end,
  playlist_track_indexes = array[playlist_a_index, playlist_b_index]
where playlists = '[]'::jsonb;
