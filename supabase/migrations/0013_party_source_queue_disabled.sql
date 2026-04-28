alter table public.party_source_queue_items
  add column disabled boolean not null default false;

create index party_source_queue_items_session_disabled_position_idx
  on public.party_source_queue_items (session_id, disabled, position);
