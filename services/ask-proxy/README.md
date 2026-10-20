# The Ask proxy and accounts service (international)

A Supabase project directory: the database migration (`profiles`, `usage_daily`, row-level
security, the sign-up trigger, the allowance functions) and three Edge Functions, `ask` (the Ask
box's model calls, with the OpenRouter key), `account` (status and Delete account) and `feedback`
(Help › Send Feedback…, posted as a GitHub Discussion; see [Feedback](#feedback) below). Design:
`docs/lm/accounts-design.md`, `docs/lm/feedback.md`.

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
   supabase secrets set ASK_MONTHLY_REQUESTS=1000 ASK_MONTHLY_TOKENS=3000000   # optional; defaults, about 300 questions
   ```
   `SUPABASE_URL`, `SUPABASE_ANON_KEY` and `SUPABASE_SERVICE_ROLE_KEY` are provided to functions
   automatically.
4. **Deploy the functions**:
   ```sh
   supabase functions deploy ask
   supabase functions deploy account
   supabase functions deploy feedback   # see Feedback below for its token
   ```
5. **Auth settings** (Dashboard › Authentication):
   - URL Configuration › Redirect URLs: add `http://127.0.0.1:*/**` (the app's loopback
     redirect). Site URL may stay as it is.
   - Sessions: JWT expiry 3600 s; refresh token rotation on, reuse interval 10 s (the defaults).
   - Sign In / Providers: turn on **Email** (passwordless: link and 6-digit code, step 6; "Confirm email" on), **Google**, **X**
     (OAuth 2.0) and **Apple** with the client ids and secrets from each provider's console
     (`docs/lm/accounts-design.md`, and the owner checklist in the accounts report). The callback
     URL each provider asks for is `https://<ref>.supabase.co/auth/v1/callback`.
   - Emails: set up custom SMTP (Resend, Postmark, Amazon SES…) before real users: the built-in
     sender is rate-limited to a few emails an hour and meant for testing.
6. **Push the sign-in emails and the code settings.** `supabase/config.toml` holds them:
   `[auth.email]` (6-digit codes, `otp_length = 6`, valid for an hour, `otp_expiry = 3600`;
   "Confirm email" on; at most one email a minute per address) and two templates in
   `supabase/templates/`, each showing the code (`{{ .Token }}`) and the link
   (`{{ .ConfirmationURL }}`): `magic_link.html` (a returning account) and `confirmation.html`
   (a first sign-in: with "Confirm email" on, Supabase sends **Confirm signup**, not Magic link,
   to an address it doesn't know yet). Both emails are needed.
   ```sh
   cd services/ask-proxy
   supabase config push        # prints each service's diff and asks before changing it
   ```
   The CLI pushes the whole Auth service at once, filling every setting `config.toml` leaves
   out with the CLI's own defaults. So **read the Auth diff before answering y**. Expected,
   for a project whose emails were never changed:
   ```diff
    [email.template.confirmation]
   -subject = "Confirm Your Signup"
   +subject = "Your NimbleMap sign-in code"
   -content = "<h2>Confirm your signup</h2> …"
   +content = "<!doctype html> … {{ .Token }} … {{ .ConfirmationURL }} …"
    [email.template.magic_link]
   -subject = "Your Magic Link"
   +subject = "Your NimbleMap sign-in code"
   -content = "<h2>Magic Link</h2> …"
   +content = "<!doctype html> … {{ .Token }} … {{ .ConfirmationURL }} …"
   ```
   plus `otp_length`/`otp_expiry`/`max_frequency`/`enable_confirmations` lines only if the
   project's values differ from 6 / 3600 / 1 minute / on. If the diff changes anything else
   (for example `[external.apple] enabled = true → false`, `site_url`, redirect URLs, rate
   limits, SMS or MFA settings, which this file doesn't describe), answer **n** and set the
   same things in the Dashboard instead:
   - Authentication › Sign In / Providers › Email: Email OTP Length **6**, Email OTP Expiration
     **3600** seconds, Confirm email **on**;
   - Authentication › Emails › Templates › **Magic link** and **Confirm signup**: subject
     "Your NimbleMap sign-in code", body = the contents of `supabase/templates/magic_link.html`
     and `supabase/templates/confirmation.html`.

   Answer **n** for the other services it offers (API, database, storage) unless you meant to
   change them. Then send yourself a sign-in from the app to a new address and to a known one:
   both emails should show a 6-digit code and the link.
7. **Write the app's config** (not committed): copy `packaging/lm/account.example.json` to a
   private place, fill in the URL and anon key, and point a build at it
   (`LM_ACCOUNT_CONFIG=/path/account.json` for a run; the bundle script copies it into
   `Resources/account.json` when `LM_ACCOUNT_CONFIG` is set).
8. **Try it**: run the Pro app with `LM_ACCOUNT_CONFIG` set, sign in from Settings › Account,
   then ask a question. `supabase functions logs ask` shows one line per call (user id, status,
   tokens), never what was asked.

## Feedback

`feedback` takes a message from the app without a sign-in and posts it as a discussion on
`kangning-huang/NimbleMap` through GitHub's GraphQL API (`createDiscussion`). The app needs no
GitHub account and no access to GitHub. Design and privacy: `docs/lm/feedback.md`.

| Form's type | Category (default id) | Title |
|---|---|---|
| Idea or suggestion | Ideas (`DIC_kwDOVO_7YM4DJIF5`) | as typed, or the first line |
| Problem or bug | Q&A (`DIC_kwDOVO_7YM4DJIF4`), so an answer can be marked | `[Problem] …` |
| Other | General (`DIC_kwDOVO_7YM4DJIF3`) | as typed, or the first line |

Function env (all optional except the token; the defaults are the ids above):

| Name | Default | What |
|---|---|---|
| `GITHUB_FEEDBACK_TOKEN` | (required) | fine-grained token, this repository only, Discussions read and write |
| `FEEDBACK_HASH_KEY` | the service role key | key of the hashes the limits are counted under |
| `FEEDBACK_REPOSITORY_ID` | `R_kgDOVO_7YA` | the repository's node id |
| `FEEDBACK_CATEGORY_IDEA` / `_PROBLEM` / `_OTHER` | Ideas / Q&A / General | category node ids |
| `FEEDBACK_DISCUSSIONS_URL` | `https://github.com/kangning-huang/NimbleMap/discussions` | answered to a honeypot hit |
| `FEEDBACK_DEVICE_DAILY` | 5 | messages per install per UTC day |
| `FEEDBACK_IP_DAILY` | 20 | messages per network address per UTC day |
| `FEEDBACK_GLOBAL_DAILY` | 200 | messages from everyone per UTC day |

Set up (owner):

1. **The token.** GitHub › Settings › Developer settings › Personal access tokens ›
   Fine-grained tokens › Generate new token: resource owner `kangning-huang`, expiry a year,
   Repository access › Only select repositories › `kangning-huang/NimbleMap`; Permissions ›
   Repository permissions › **Discussions: Read and write** (Metadata: Read-only is added by
   GitHub itself); nothing else.
2. **Store it** (from this folder, linked to the project):
   ```sh
   supabase secrets set GITHUB_FEEDBACK_TOKEN=<the token>
   supabase secrets set FEEDBACK_HASH_KEY=$(openssl rand -hex 32)   # optional, recommended
   ```
3. **Push the tables and deploy**:
   ```sh
   supabase db push                       # 20261020000000_feedback.sql
   supabase functions deploy feedback     # verify_jwt = false comes from config.toml
   ```
4. **Try it** without posting: `curl -s -X POST https://<ref>.supabase.co/functions/v1/feedback
   -H 'Content-Type: application/json' -d '{"kind":"other"}'` answers `invalid`. A real test
   posts a real discussion: delete it afterwards.

Reading reply addresses: Table Editor › `feedback_contacts` (or SQL `select * from
public.feedback_contacts order by created_at desc;`); delete a row once the conversation is over.
`supabase functions logs feedback` shows one line per message (kind, status, whether an address
was kept), never the text, the address or a hash.

## Housekeeping

- Usage older than 90 days isn't needed: schedule
  `delete from public.usage_daily where day < current_date - 90;` daily (Database › Cron).
- Feedback counters older than a week aren't needed:
  `delete from public.feedback_rate where day < current_date - 7;` daily (Database › Cron).
- To give or take away Pro by hand: `update public.profiles set pro = false where id = '<uuid>';`
  in the SQL editor (service role).
- A rotated OpenRouter key: `supabase secrets set OPENROUTER_API_KEY=…`; functions pick it up on
  their next start (redeploy to force it).
