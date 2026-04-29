alter table public.party_sessions
  add column source_insert_interval integer not null default 0,
  add constraint party_sessions_source_insert_interval_check
    check (source_insert_interval >= 0 and source_insert_interval <= 25);

alter table public.party_queue_items
  add column from_source_queue boolean not null default false;
