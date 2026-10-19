-- NimbleMap accounts and the Ask proxy's allowance (docs/lm/accounts-design.md).
-- Per user only what the plan allows: the sign-in service's id, how they signed in, an email when
-- the provider gives one, `pro`, the sign-up date, later the plan. No names, avatars or map data.

-- ── Profiles ──────────────────────────────────────────────────────────────────────────────────
create table public.profiles (
  id uuid primary key references auth.users (id) on delete cascade,
  provider text not null default 'email',
  email text,
  -- Pro is free for anyone who registers, at first (owner, 2026-10-19).
  pro boolean not null default true,
  created_at timestamptz not null default now(),
  -- A subscription later: a payment webhook (service role) sets these.
  plan text,
  renews_at timestamptz
);

alter table public.profiles enable row level security;

-- A signed-in user reads their own row and nothing else; nobody writes through the API.
create policy "profiles: read own row" on public.profiles
  for select to authenticated using ((select auth.uid()) = id);

revoke all on public.profiles from anon;
revoke insert, update, delete, truncate on public.profiles from authenticated;

-- The profile is made on sign-up. `security definer` so it can write past RLS; an empty
-- search_path so nothing can shadow the names it uses.
create function public.handle_new_user() returns trigger
language plpgsql security definer set search_path = '' as $$
begin
  insert into public.profiles (id, provider, email)
  values (new.id, coalesce(new.raw_app_meta_data ->> 'provider', 'email'), new.email);
  return new;
end;
$$;

create trigger on_auth_user_created
  after insert on auth.users
  for each row execute function public.handle_new_user();

revoke execute on function public.handle_new_user() from public, anon, authenticated;

-- ── Daily allowance ───────────────────────────────────────────────────────────────────────────
-- One row per user and UTC day: model calls and the tokens they reported. Nothing about what was
-- asked is stored.
create table public.usage_daily (
  user_id uuid not null references auth.users (id) on delete cascade,
  day date not null,
  requests integer not null default 0,
  prompt_tokens bigint not null default 0,
  completion_tokens bigint not null default 0,
  primary key (user_id, day)
);

alter table public.usage_daily enable row level security;

create policy "usage: read own rows" on public.usage_daily
  for select to authenticated using ((select auth.uid()) = user_id);

revoke all on public.usage_daily from anon;
revoke insert, update, delete, truncate on public.usage_daily from authenticated;

-- Adds one request to the day unless the day's requests or tokens are used up, atomically (the
-- row lock of the update serialises two requests at once). Returns whether it was allowed and the
-- day's totals after it.
create function public.reserve_request(p_user uuid, p_day date, p_max_requests integer, p_max_tokens bigint)
returns table (allowed boolean, requests integer, tokens bigint)
language plpgsql security definer set search_path = '' as $$
declare
  r public.usage_daily;
begin
  insert into public.usage_daily (user_id, day) values (p_user, p_day) on conflict do nothing;
  update public.usage_daily u
     set requests = u.requests + 1
   where u.user_id = p_user and u.day = p_day
     and u.requests < p_max_requests
     and u.prompt_tokens + u.completion_tokens < p_max_tokens
  returning * into r;
  if found then
    return query select true, r.requests, r.prompt_tokens + r.completion_tokens;
  else
    select * into r from public.usage_daily u where u.user_id = p_user and u.day = p_day;
    return query select false, r.requests, r.prompt_tokens + r.completion_tokens;
  end if;
end;
$$;

-- Adds the tokens an answer reported.
create function public.record_tokens(p_user uuid, p_day date, p_prompt bigint, p_completion bigint)
returns void
language plpgsql security definer set search_path = '' as $$
begin
  update public.usage_daily u
     set prompt_tokens = u.prompt_tokens + greatest(p_prompt, 0),
         completion_tokens = u.completion_tokens + greatest(p_completion, 0)
   where u.user_id = p_user and u.day = p_day;
end;
$$;

-- Only the Ask function (service role) may call these: Supabase grants execute on new functions
-- to anon and authenticated by default, which would let a user reset their own allowance.
revoke execute on function public.reserve_request(uuid, date, integer, bigint) from public, anon, authenticated;
revoke execute on function public.record_tokens(uuid, date, bigint, bigint) from public, anon, authenticated;
grant execute on function public.reserve_request(uuid, date, integer, bigint) to service_role;
grant execute on function public.record_tokens(uuid, date, bigint, bigint) to service_role;

-- Old usage isn't needed: keep 90 days. Run daily with pg_cron (Database › Cron), or by hand:
--   delete from public.usage_daily where day < current_date - 90;
