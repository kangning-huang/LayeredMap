-- A monthly allowance beside the daily one (owner, 2026-10-19: about 300 questions a month while
-- Pro is free; docs/lm/app-store.md › Pricing). The month is the UTC calendar month of p_day,
-- summed from usage_daily; today's row lock still serialises one user's requests, so the check
-- and the increment stay atomic. `blocked` says which allowance stopped a request: 'day',
-- 'month' or null.

drop function public.reserve_request(uuid, date, integer, bigint);

create function public.reserve_request(
  p_user uuid,
  p_day date,
  p_max_requests integer,
  p_max_tokens bigint,
  p_max_month_requests integer,
  p_max_month_tokens bigint
)
returns table (allowed boolean, requests integer, tokens bigint, month_requests integer, month_tokens bigint, blocked text)
language plpgsql security definer set search_path = '' as $$
declare
  r public.usage_daily;
  m_requests integer;
  m_tokens bigint;
begin
  insert into public.usage_daily (user_id, day) values (p_user, p_day) on conflict do nothing;
  -- Lock today's row first, so two requests at once see each other's counts.
  select * into r from public.usage_daily u where u.user_id = p_user and u.day = p_day for update;
  select coalesce(sum(u.requests), 0), coalesce(sum(u.prompt_tokens + u.completion_tokens), 0)
    into m_requests, m_tokens
    from public.usage_daily u
   where u.user_id = p_user
     and u.day >= date_trunc('month', p_day)::date
     and u.day <= p_day;
  if r.requests >= p_max_requests or r.prompt_tokens + r.completion_tokens >= p_max_tokens then
    return query select false, r.requests, r.prompt_tokens + r.completion_tokens, m_requests, m_tokens, 'day'::text;
  elsif m_requests >= p_max_month_requests or m_tokens >= p_max_month_tokens then
    return query select false, r.requests, r.prompt_tokens + r.completion_tokens, m_requests, m_tokens, 'month'::text;
  else
    update public.usage_daily u set requests = u.requests + 1
     where u.user_id = p_user and u.day = p_day
    returning * into r;
    return query select true, r.requests, r.prompt_tokens + r.completion_tokens, m_requests + 1, m_tokens, null::text;
  end if;
end;
$$;

revoke execute on function public.reserve_request(uuid, date, integer, bigint, integer, bigint) from public, anon, authenticated;
grant execute on function public.reserve_request(uuid, date, integer, bigint, integer, bigint) to service_role;
