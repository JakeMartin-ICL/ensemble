alter table public.party_queue_items
  drop constraint if exists party_queue_items_session_id_position_key;

with ranked as (
  select id, row_number() over (
    partition by session_id
    order by position asc, created_at asc
  ) - 1 as new_position
  from public.party_queue_items
)
update public.party_queue_items q
set position = ranked.new_position
from ranked
where q.id = ranked.id;
