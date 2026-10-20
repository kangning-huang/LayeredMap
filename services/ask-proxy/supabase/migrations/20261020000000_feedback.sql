-- In-app feedback (docs/lm/feedback.md): the `feedback` function posts each message from the
-- app's Help › Send Feedback… as a GitHub Discussion. Nobody signs in, so the function keeps two
-- private tables, both for the service role only (row level security on, no policies, nothing
-- granted to anon or authenticated):
--   feedback_rate      daily counters per install, per network address and overall, keyed by a
--                      keyed hash (HMAC) of the install id or the address, never the values;
--   feedback_contacts  the reply address someone chose to give, beside the discussion it belongs
--                      to, so the owner can answer. It is never posted.

-- ── Daily counters ────────────────────────────────────────────────────────────────────────────
create table public.feedback_rate (
  -- 'device:<hmac>', 'ip:<hmac>' or 'all'.
  bucket text not null,
  day date not null,
  sent integer not null default 0,
  primary key (bucket, day)
);

alter table public.feedback_rate enable row level security;

revoke all on public.feedback_rate from anon, authenticated;

-- ── Reply addresses ───────────────────────────────────────────────────────────────────────────
create table public.feedback_contacts (
  id bigint generated always as identity primary key,
  created_at timestamptz not null default now(),
  discussion_url text not null,
  email text not null check (char_length(email) <= 254)
);

alter table public.feedback_contacts enable row level security;

revoke all on public.feedback_contacts from anon, authenticated;

-- Counts one message for the install, the address and everyone on p_day, unless one of them is
-- used up, atomically. The rows are locked in one order (install, address, all), so two requests
-- never wait on each other. `blocked` says which limit stopped it: 'device', 'ip', 'all' or null.
create function public.feedback_reserve(
  p_device text,
  p_ip text,
  p_day date,
  p_max_device integer,
  p_max_ip integer,
  p_max_all integer
)
returns table (allowed boolean, blocked text)
language plpgsql security definer set search_path = '' as $$
declare
  d integer;
  i integer;
  a integer;
begin
  insert into public.feedback_rate (bucket, day)
  values (p_device, p_day), (p_ip, p_day), ('all', p_day)
  on conflict do nothing;
  select r.sent into d from public.feedback_rate r where r.bucket = p_device and r.day = p_day for update;
  select r.sent into i from public.feedback_rate r where r.bucket = p_ip and r.day = p_day for update;
  select r.sent into a from public.feedback_rate r where r.bucket = 'all' and r.day = p_day for update;
  if d >= p_max_device then
    return query select false, 'device'::text;
  elsif i >= p_max_ip then
    return query select false, 'ip'::text;
  elsif a >= p_max_all then
    return query select false, 'all'::text;
  else
    update public.feedback_rate r set sent = r.sent + 1
     where r.day = p_day and r.bucket in (p_device, p_ip, 'all');
    return query select true, null::text;
  end if;
end;
$$;

-- Only the feedback function (service role) may call it: Supabase grants execute on new functions
-- to anon and authenticated by default.
revoke execute on function public.feedback_reserve(text, text, date, integer, integer, integer) from public, anon, authenticated;
grant execute on function public.feedback_reserve(text, text, date, integer, integer, integer) to service_role;

-- Old counters aren't needed: keep a week. Run daily with pg_cron (Database › Cron), or by hand:
--   delete from public.feedback_rate where day < current_date - 7;
-- Reply addresses: delete a row once the conversation is over, or after a year at most:
--   delete from public.feedback_contacts where created_at < now() - interval '1 year';
