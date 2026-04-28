do $$
begin
  if to_regclass('public.car_sessions') is not null
    and to_regclass('public.weave_sessions') is null then
    alter table public.car_sessions rename to weave_sessions;
  end if;
end $$;

do $$
begin
  if exists (
    select 1
    from pg_type t
    join pg_namespace n on n.oid = t.typnamespace
    where n.nspname = 'public' and t.typname = 'car_turn'
  ) and not exists (
    select 1
    from pg_type t
    join pg_namespace n on n.oid = t.typnamespace
    where n.nspname = 'public' and t.typname = 'weave_turn'
  ) then
    alter type public.car_turn rename to weave_turn;
  end if;
end $$;

alter index if exists public.car_sessions_pkey
  rename to weave_sessions_pkey;

do $$
begin
  if not exists (
    select 1 from pg_policies
    where schemaname = 'public'
      and tablename = 'weave_sessions'
      and policyname = 'Host can read own weave sessions'
  ) then
    if exists (
      select 1 from pg_policies
      where schemaname = 'public'
        and tablename = 'weave_sessions'
        and policyname = 'Host can read own car sessions'
    ) then
      alter policy "Host can read own car sessions"
        on public.weave_sessions
        rename to "Host can read own weave sessions";
    elsif exists (
      select 1 from pg_policies
      where schemaname = 'public'
        and tablename = 'weave_sessions'
        and policyname = 'Host can read own sessions'
    ) then
      alter policy "Host can read own sessions"
        on public.weave_sessions
        rename to "Host can read own weave sessions";
    end if;
  end if;
end $$;

do $$
begin
  if not exists (
    select 1 from pg_policies
    where schemaname = 'public'
      and tablename = 'weave_sessions'
      and policyname = 'Host can update own weave sessions'
  ) then
    if exists (
      select 1 from pg_policies
      where schemaname = 'public'
        and tablename = 'weave_sessions'
        and policyname = 'Host can update own car sessions'
    ) then
      alter policy "Host can update own car sessions"
        on public.weave_sessions
        rename to "Host can update own weave sessions";
    elsif exists (
      select 1 from pg_policies
      where schemaname = 'public'
        and tablename = 'weave_sessions'
        and policyname = 'Host can update own sessions'
    ) then
      alter policy "Host can update own sessions"
        on public.weave_sessions
        rename to "Host can update own weave sessions";
    end if;
  end if;
end $$;
