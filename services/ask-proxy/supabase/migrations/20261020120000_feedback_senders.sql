-- In-app feedback from signed-in users (docs/lm/feedback.md): when the app sends the session's
-- access token with a message, the `feedback` function verifies it and keeps the account id here,
-- beside the discussion it belongs to, so the owner can follow up. The id is never posted on
-- GitHub (the post says only "From a signed-in user"). Service role only, like feedback_contacts:
-- row level security on, no policies, nothing granted to anon or authenticated.

create table public.feedback_senders (
  discussion_url text primary key,
  -- A deleted account takes its rows with it.
  user_id uuid not null references auth.users (id) on delete cascade,
  created_at timestamptz not null default now()
);

alter table public.feedback_senders enable row level security;

revoke all on public.feedback_senders from anon, authenticated;

-- Who sent a discussion (SQL editor, service role):
--   select s.discussion_url, s.user_id, u.email, s.created_at
--     from public.feedback_senders s join auth.users u on u.id = s.user_id
--    where s.discussion_url = 'https://github.com/kangning-huang/NimbleMap/discussions/<n>';
-- Keep it as long as a reply address at most: delete after a year.
--   delete from public.feedback_senders where created_at < now() - interval '1 year';
