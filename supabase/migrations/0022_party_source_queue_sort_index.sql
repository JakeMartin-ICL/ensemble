create index party_source_queue_items_session_sort_idx
  on public.party_source_queue_items (
    session_id,
    disabled,
    (position < 0),
    position,
    created_at
  );
