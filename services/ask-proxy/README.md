# The Ask proxy and accounts service (international)

A Supabase project directory: the database migration (`profiles`, `usage_daily`, row-level
security, the sign-up trigger, the allowance functions) and two Edge Functions, `ask` (the Ask
box's model calls, with the OpenRouter key) and `account` (status and Delete account). Design:
`docs/lm/accounts-design.md`.

No user data, keys or project ids are kept in this repository. `.env.example` lists the secrets
by name only.

## Test the logic

The functions' logic lives in `supabase/functions/_shared/` and runs under Node (22.18 or later
strips the TypeScript types itself); `index.ts` only connects it to Deno and Supabase.

```sh
cd services/ask-proxy && npm test
```

## Deploy (owner)

Needs the Supabase CLI (`brew install supabase/tap/supabase`, or `npx supabase …` without
installing) and a Supabase account. Docker is only needed for a local stack (`supabase start`),
not for deploying.

1. **Create the project** at supabase.com (Dashboard › New project). Pick a region near most
   users (e.g. `us-east-1` or `eu-central-1`); note the project ref (the `xxxx` in
   `https://xxxx.supabase.co`).
2. **Link and push the schema**:
   ```sh
   cd services/ask-proxy
   supabase login
   supabase link --project-ref <ref>
   supabase db push            # runs supabase/migrations/*.sql
   ```
3. **Set the function secrets** (never in a file in this repo):
   ```sh
   supabase secrets set OPENROUTER_API_KEY=<a new key made for the proxy, with a credit limit>
   supabase secrets set ASK_DAILY_REQUESTS=60 ASK_DAILY_TOKENS=400000   # optional; these are the defaults
   ```
   `SUPABASE_URL`, `SUPABASE_ANON_KEY` and `SUPABASE_SERVICE_ROLE_KEY` are provided to functions
   automatically.
4. **Deploy the functions**:
   ```sh
   supabase functions deploy ask
   supabase functions deploy account
   ```
5. **Auth settings** (Dashboard › Authentication):
   - URL Configuration › Redirect URLs: add `http://127.0.0.1:*/**` (the app's loopback
     redirect). Site URL may stay as it is.
   - Sessions: JWT expiry 3600 s; refresh token rotation on, reuse interval 10 s (the defaults).
   - Sign In / Providers: turn on **Email** (magic link; "Confirm email" on), **Google**, **X**
     (OAuth 2.0) and **Apple** with the client ids and secrets from each provider's console
     (`docs/lm/accounts-design.md`, and the owner checklist in the accounts report). The callback
     URL each provider asks for is `https://<ref>.supabase.co/auth/v1/callback`.
   - Emails: set up custom SMTP (Resend, Postmark, Amazon SES…) before real users: the built-in
     sender is rate-limited to a few emails an hour and meant for testing.
6. **Write the app's config** (not committed): copy `packaging/lm/account.example.json` to a
   private place, fill in the URL and anon key, and point a build at it
   (`LM_ACCOUNT_CONFIG=/path/account.json` for a run; the bundle script copies it into
   `Resources/account.json` when `LM_ACCOUNT_CONFIG` is set).
7. **Try it**: run the Pro app with `LM_ACCOUNT_CONFIG` set, sign in from Settings › Account,
   then ask a question. `supabase functions logs ask` shows one line per call (user id, status,
   tokens), never what was asked.

## Housekeeping

- Usage older than 90 days isn't needed: schedule
  `delete from public.usage_daily where day < current_date - 90;` daily (Database › Cron).
- To give or take away Pro by hand: `update public.profiles set pro = false where id = '<uuid>';`
  in the SQL editor (service role).
- A rotated OpenRouter key: `supabase secrets set OPENROUTER_API_KEY=…`; functions pick it up on
  their next start (redeploy to force it).
