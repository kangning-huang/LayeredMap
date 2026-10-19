# Accounts and the Ask proxy: design (WP7.3, WP7.4, WP7.6)

The plan is `docs/lm/accounts.md` (owner decisions of 2026-10-19). This page is how it is built:
endpoints, tokens, the redirect, offline, sign out and delete, regions, and how the Ask box moves
from the key in the test build to the proxy. Code: `crates/lm-account` (the app's side),
`crates/lm-ask/src/proxy.rs` (the Ask box's transport), `crates/lm-ui/src/account_ui.rs`
(Settings › Account and the sign-in prompt), `services/ask-proxy` (the server, international).

## Pieces

```
 app (lm-account)            system browser              Supabase (international)
 ───────────────            ──────────────              ────────────────────────
 PKCE verifier + state ──▶  /auth/v1/authorize  ──▶  Apple / Google / X  ──▶  Auth
 127.0.0.1:<port> ◀──────── 302 …/callback?state=…&code=…
 POST /auth/v1/token?grant_type=pkce ───────────────────────────────────────▶  Auth
   ◀── access token (JWT, 1 h) + refresh token (rotating)
 refresh token → Keychain / Credential Manager

 Ask box (lm-ask proxy) ── POST /functions/v1/ask, Bearer <JWT> ──▶ Edge Function `ask`
                                                                    ├ verify JWT, read profiles.pro
                                                                    ├ reserve today's allowance (Postgres)
                                                                    └ OpenRouter (key: function secret) ─ streamed back
```

## Endpoints (international)

`SUPABASE_URL` is the project's URL (`https://<project-ref>.supabase.co`, or a custom domain
later). Every Auth call carries `apikey: <anon key>`; the anon (publishable) key is public by
design and ships in the app's config. Row-level security is what protects data.

| What | Request | Notes |
|---|---|---|
| Sign in with a provider | browser → `GET {SUPABASE_URL}/auth/v1/authorize?provider=<apple\|google\|x>&redirect_to=<loopback>&code_challenge=<S256>&code_challenge_method=s256` | Supabase runs the provider's own OAuth and sends the browser to `redirect_to` with `?code=`. Provider ids come from the config (X is `x` for OAuth 2.0, `twitter` for the legacy OAuth 1.0a provider: use whichever the project enables) |
| Email (link and code) | `POST {SUPABASE_URL}/auth/v1/otp?redirect_to=<loopback>` `{"email", "create_user": true, "code_challenge", "code_challenge_method": "s256"}` | One email with a link and a 6-digit code (see Email: the link or the code). The link opens the browser, which lands on the loopback with `?code=` like a provider |
| Email code → tokens | `POST {SUPABASE_URL}/auth/v1/verify` `{"type": "email", "email", "token": "<6 digits>"}` | Same answer as the code exchange below. A wrong, expired or used code: 403 `otp_expired`; too many attempts: 429 |
| Code → tokens | `POST {SUPABASE_URL}/auth/v1/token?grant_type=pkce` `{"auth_code", "code_verifier"}` | Answers `access_token`, `expires_in`, `expires_at`, `refresh_token`, `user` (`id`, `email`, `app_metadata.provider`) |
| Refresh | `POST {SUPABASE_URL}/auth/v1/token?grant_type=refresh_token` `{"refresh_token"}` | Same answer; the refresh token is replaced (rotation) |
| Sign out | `POST {SUPABASE_URL}/auth/v1/logout?scope=local` with `Authorization: Bearer <access>` | Revokes this device's refresh token |
| Ask | `POST {SUPABASE_URL}/functions/v1/ask` with `Authorization: Bearer <access>`; body = the chat-completions body `lm-ask` builds today | Answers OpenRouter's answer (JSON, or SSE when the body asks `stream: true`); errors are JSON `{"error": {"code", "message"}}` |
| Account status | `GET {SUPABASE_URL}/functions/v1/account` | `{"pro", "plan", "today": {"requests", "tokens"}, "limits": {...}}` |
| Delete account | `DELETE {SUPABASE_URL}/functions/v1/account` | The function deletes the Auth user with the service-role key; `profiles` and `usage_daily` rows go with it (`on delete cascade`) |

## Tokens

| Token | Lifetime | Where it lives |
|---|---|---|
| PKCE verifier and `state` | one sign-in (at most 10 minutes) | memory only |
| Authorization code | single use, a few minutes (Supabase) | passes through the loopback request only |
| Email code (6 digits) | single use, 1 hour (`otp_expiry`) | the email, then typed into the app; sent once to `/auth/v1/verify`, never stored |
| Access token (JWT) | 1 hour (Supabase default; keep it) | memory only; never written, never logged |
| Refresh token | until used (rotation: each refresh returns a new one; reuse after 10 s revokes the session) | macOS Keychain (generic password, service `app.layeredmap.pro.account`, account `session`) or Windows Credential Manager (Web Credentials, resource `app.layeredmap.pro.account`) |

The stored item is one hex-encoded JSON record: region, refresh token, user id, sign-in method,
email. Hex keeps it safe to hand to the `security` tool and PowerShell on standard input (never
on a command line). The email is kept so Settings › Account can say who is signed in without a
network call (and offline).

The app refreshes when the access token has less than 5 minutes left, before a request needs it
(not on a timer), and writes the new refresh token back. A refresh answered 400/401 ("invalid
refresh token": revoked, reused or the user deleted) signs the app out locally and says so.
Owner option (Supabase Pro plan): Auth › Sessions › time-box sessions (e.g. 90 days) and an
inactivity timeout (e.g. 30 days).

**Storage is injectable.** `lm_account::store::SecretStore` has a Keychain implementation
(`/usr/bin/security`, as the OpenRouter key already uses), a Credential Manager implementation
(Windows PowerShell's `Windows.Security.Credentials.PasswordVault`, a WinRT class it can load
without any unsafe Win32 call in our code) and an in-memory one, which every test uses: no test
writes to the login keychain or raises a permission dialog.

## The redirect: loopback on both systems

RFC 8252 allows a private-use URL scheme (§7.1), a claimed https link (§7.2) or a loopback
address (§7.3). NimbleMap uses **loopback, `http://127.0.0.1:<random port>/callback`, on macOS and
on Windows**:

- It needs nothing registered with the system: no `CFBundleURLTypes` in the Info.plist, no URL
  protocol keys written by the Inno Setup installer, so unsigned test builds, `cargo run` and a
  copy run from a disk image all sign in the same way.
- The callback is an ordinary socket read in safe Rust (`std::net::TcpListener`). A URL scheme on
  macOS arrives through the app delegate's `application:openURLs:`, which the app's macOS module
  would have to route into egui with more `unsafe` code; on Windows it arrives as a second
  process launch that has to find the first one.
- A private-use scheme can be claimed by any other app on the machine; a loopback port is bound
  by us for the length of one sign-in. PKCE protects the code either way.
- Binding to `127.0.0.1` (never `0.0.0.0`) doesn't trigger the macOS or Windows firewall prompt.
- `127.0.0.1` rather than `localhost`, as RFC 8252 §8.3 recommends (no resolver, no IPv6 surprise).

The app binds port 0 (the system picks a free port), puts its own `state` into `redirect_to`'s
query (`…/callback?state=<random>`, which Supabase keeps and to which it appends `code=`), opens
the browser, and waits up to 10 minutes. A request with the wrong `state`, or without one, gets an
error page and doesn't end the wait; `error=` from the provider ends it with the provider's
reason. The page the browser shows afterwards says to go back to the app; it loads nothing
external.

Limits: the email *link* must be opened on the same computer within those 10 minutes (opened on a
phone, it can't reach the app's loopback). The email's 6-digit code has no such limit: it can be
read anywhere (a phone included) and typed into the app for an hour.

## Email: the link or the code

Email sign-in is passwordless (no passwords anywhere). `Account::begin` asks Supabase for the
email (`create_user: true`: the first sign-in registers the account) and binds the loopback; the
email carries both the link and a 6-digit code. Settings › Account then says "We sent an email to
{address}. Click the link in it on this Mac, or enter the 6-digit code." with a code field
(digits only; a pasted "123 456" or "123-456" is taken as 123456), Verify (Return), Send a New
Email and Use a Different Email.

- **The link** works as before: same computer, within the 10-minute loopback wait.
- **The code**: `Account::verify_code(region, email, code)` strips spaces and dashes, requires six
  digits before sending, then `POST /auth/v1/verify` `{"type": "email", "email", "token"}`; the
  answer is kept exactly like the link's (credential store, then the interface). A 400/403
  ("Token has expired or is invalid") reads "That code didn't work. Check it, or send a new
  email."; 429 reads "Too many attempts. Wait a few minutes, then try again." `type: "email"`
  matches both emails Supabase may send: **Magic link** for a known address and **Confirm
  signup** for a first sign-in ("Confirm email" is on), so both templates show the code.
- **Whichever comes first wins.** The sign-in waiting for its link is registered in `Account`
  (an id and a stop flag). A code that verifies sets the flag; the loopback wait sees it within
  50 ms, closes the listener and answers `Cancelled`, so no listener thread outlives the sign-in.
  Finishing (the link's code exchange, or the verify request) happens under one lock, so only one
  of them can keep a session; a code typed after the link already signed in sends nothing. A
  wrong code leaves the link waiting. Send a New Email starts a new sign-in, which stops the older
  wait the same way.
- After the link's 10 minutes the pane keeps the code field ("The link has stopped working on
  this Mac; the code still works for an hour.").

The project side is in `services/ask-proxy/supabase/config.toml` (`[auth.email]`: `otp_length = 6`,
`otp_expiry = 3600`, Confirm email on, one email a minute per address) and the two templates in
`services/ask-proxy/supabase/templates/` (subject "Your NimbleMap sign-in code"; the code large,
the link, and a line that the email can be ignored by someone who didn't ask for it). The owner
pushes them with `supabase config push` (`services/ask-proxy/README.md`, step 6).

## Regions

`lm_account::Region` is `International` or `China`. The default comes from the system region the
app already reads for Pro's borders (`lm_ask::reference::system_region`): `CN` → China;
everything else, including HK, MO and TW, → International. The signed-out Account pane shows the
region and lets the person switch it before signing in (for that sign-in; the next launch starts
from the system region again); once signed in, the session's region (kept in the stored record)
is the one used, and switching means signing out first.
Accounts are not merged across regions.

Each region has its own config block (auth URL, anon key or client id, proxy URL, providers). A
region whose block is missing or still holds placeholders shows "Sign-in isn't available in this
build yet"; the China block stays empty until the China service exists.

## Configuration

Read in this order, first found wins per region:

1. `LM_ACCOUNT_CONFIG=<path>` (development);
2. `account.json` in the app's resources (`Contents/Resources/account.json`, `Resources\account.json`
   beside the exe), written by the bundle script / installer from the owner's local copy;
3. nothing: accounts are off, and the Ask box behaves as today.

`packaging/lm/account.example.json` holds placeholders only. The real file holds nothing secret
(the URL and anon key are public by design) but stays out of git so test projects don't leak
into history.

## Offline

Signing in, refreshing, signing out on the server and deleting are network requests for the new
`lm_net::Purpose::Account`; strict offline mode (View › Work Offline) refuses them like every
other purpose. Offline the Account pane still shows who is signed in (from the stored record),
disables the sign-in buttons and Delete account with a line saying why, and Sign out still clears
the local token (the server-side revocation is skipped and the token simply expires unused). The
Ask box already says it can't reach the assistant offline.

## Sign out and delete account

- **Sign out**: best-effort `POST /auth/v1/logout?scope=local`, then the stored record and the
  in-memory tokens are deleted, whatever the server said.
- **Delete account**: a confirmation ("Delete your NimbleMap account? Pro stops working on every
  computer you use. Your maps and files are not affected."), then `DELETE /functions/v1/account`.
  The function deletes the Auth user (`auth.admin.deleteUser`), which cascades to `profiles` and
  `usage_daily`; the app then clears its record. Nothing about the person remains apart from what
  the providers and Supabase keep in their own logs for their retention period. Map data was never
  sent.

## The Ask box: which route

`lm_account::route::AskRoute` decides once per request:

| Situation | Route |
|---|---|
| `LM_ASK_ROUTE=direct` and a developer key is present (keychain / `OPENROUTER_API_KEY` / `assistant.dat`) | **Direct** to OpenRouter (development only) |
| An account config with a proxy URL for the region | **Proxy**: needs a signed-in account; signed out, the Ask box shows "Sign in to use NimbleMap Pro" with a Sign in… button that opens Settings › Account |
| No account config, a key present (today's Pro test builds) | **Direct** (unchanged behaviour) |
| Neither | Unavailable ("The assistant isn't set up in this build") |

`LM_ASK_ROUTE=proxy` forces the proxy even when a key is present (to test the proxy from a build
that still has a key). The body the Ask box sends is the one `lm-ask` builds today, byte for byte
(a test checks that the proxy transport sends exactly what the direct one would), so the privacy
rule holds unchanged: the model sees layer names, kinds, fields, extents, filters and counts,
never feature values, except the few `resolve_feature` matches (`lm-ask/src/context.rs`,
`resolve.rs`). The proxy forwards only allow-listed keys and stores no request or answer, only
per-user daily counts.

**When the test-build key goes.** After (1) the owner's Supabase project is live, (2) the `ask`
function is deployed with the OpenRouter key as its secret and a Pro build signed in and asked
through it, and (3) one shared Pro build has run through the proxy for a week: remove
`bundled_key`, the `PAD`, `internal_build`'s use of it, the `assistant.dat` steps in
`packaging/lm/bundle-macos.sh` and `packaging/lm/windows/build-installer.ps1` (`LM_OPENROUTER_KEY`),
and the "Direct" fallback for builds without an account config. The developer path
(`LM_ASK_ROUTE=direct` with the keychain key) stays for prompt work and the evaluation harness.
The cost caption (internal builds only) then shows for `LM_DEV=1` only.

## The server (international)

`services/ask-proxy` holds a Supabase project directory: migrations and two Edge Functions
(Deno/TypeScript). The logic is in plain modules (`functions/_shared/*.ts`) tested with Node's
built-in runner; the two `index.ts` files only wire Deno, supabase-js and `fetch` to it.

- `profiles(id → auth.users on delete cascade, provider, email, pro default true, created_at,
  plan, renews_at)`, created by a trigger on `auth.users` insert. RLS: a user may `select` only
  their own row; nobody may write through the API (only the service role and the trigger).
- `usage_daily(user_id, day, requests, prompt_tokens, completion_tokens)`, primary key
  `(user_id, day)`. RLS: select own rows only. Written only by `reserve_request()` and
  `record_tokens()`, `security definer` functions callable by the service role alone.
- `ask`: verifies the JWT (signature, `aud = authenticated`, expiry; gateway `verify_jwt` on as
  well), reads `profiles.pro`, calls `reserve_request(uid, today_utc, max_requests, max_tokens)`,
  which atomically adds one request unless the day's requests or tokens are used up, shapes the
  body (allow-listed keys, allowed models, `max_tokens` clamped, size capped, `usage.include`
  forced), forwards it to OpenRouter with `OPENROUTER_API_KEY` from the function's secrets,
  streams the answer back, and records the tokens the answer reports. Limits come from secrets:
  `ASK_DAILY_REQUESTS` (default 60 model calls, about 20 questions) and `ASK_DAILY_TOKENS`
  (default 400,000) per UTC day, and `ASK_MONTHLY_REQUESTS` (default 1,000 calls, about 300
  questions) and `ASK_MONTHLY_TOKENS` (default 3,000,000) per UTC month (owner, 2026-10-19). Over
  the month's allowance the answer is 429 `monthly_limit` with the seconds until the first of next
  month; the Ask box says "You've used this month's assistant allowance. It renews in N days."
- `account`: `GET` status, `DELETE` the user.

Deploy steps: `services/ask-proxy/README.md`.

## Mainland China (design only)

Same contract, different services, all hosted in mainland China (PIPL: China accounts stay on
China servers, so no cross-border transfer).

**Identity service adapter.** `lm-account` talks to an identity provider through one shape,
`AuthBackend`:

| Operation | International (Supabase) | China (OIDC identity service: Authing, Tencent CloudBase or Alibaba Cloud IDaaS) |
|---|---|---|
| authorize URL | `/auth/v1/authorize?provider=…` | the service's OIDC `/authorize` (or `/oidc/auth`) with `client_id`, `response_type=code`, `scope=openid phone offline_access`, `redirect_uri` (loopback), `state`, `nonce`, PKCE S256; the service's hosted login page offers WeChat QR (微信扫码, the website flow), Alipay (支付宝扫码) and SMS code (手机号验证码), plus Sign in with Apple |
| code → tokens | `/auth/v1/token?grant_type=pkce` | `/token` with `grant_type=authorization_code`, `code`, `code_verifier`, `client_id`; checks the ID token's `nonce`, `iss`, `aud` |
| refresh | `grant_type=refresh_token` | `grant_type=refresh_token` |
| sign out | `/auth/v1/logout` | the service's revocation endpoint (`/token/revocation`) |
| delete | `DELETE /functions/v1/account` | `DELETE` on the China proxy's `/account`, which calls the service's management API |

Authing, CloudBase and IDaaS all expose standard OIDC with PKCE for "native"/"public" clients
and host the WeChat, Alipay and SMS logins behind their own login page, so the app never embeds a
vendor SDK: the China backend is the same browser + loopback flow with different URLs and an ID
token to check. Only the China config block (`issuer`, `client_id`, `proxy_url`, `providers` for
the buttons, which become `login_hint`/`provider` parameters where the service supports
skipping its picker) and the token response mapping differ. In code this becomes an
`AuthBackend` enum (`Supabase`, `Oidc`) inside `lm-account`, chosen by the region's config, so the
app keeps one `Account` type; today `Account` speaks Supabase only, and the China region shows
"coming in a later version". The `Oidc` arm adds: the issuer's discovery document
(`/.well-known/openid-configuration`) for the endpoints, a `nonce` beside `state` (generated like
`state`, checked in the ID token), ID-token signature checking against the issuer's JWKS, and
`client_id` in place of `apikey`.

**Proxy.** A function on the China cloud (Tencent SCF / CloudBase functions or Alibaba Function
Compute) with the same HTTP contract as `ask`: `POST /ask` with the chat-completions body,
`Bearer` token from the China identity service (verified against its JWKS), the same `profiles` /
`usage_daily` tables in a China-hosted Postgres or MySQL, the same allow-list shaping, forwarding
to a **filed domestic model** (备案: e.g. Qwen through Alibaba Model Studio / DashScope's
OpenAI-compatible endpoint, DeepSeek, Zhipu GLM, Moonshot Kimi, all with OpenAI-style tool
calling) instead of OpenRouter. AI output is labelled in the app's result card ("AI-generated
plan", already implied by the recipe) as the Interim Measures require. The model id the app sends
is mapped by the proxy, so the app sends the same body to both regions. The domestic model must
pass the evaluation set (`docs/lm/llm-eval.md`, Gate 7 thresholds) before it is used.

**Order.** China sign-in can't start before the owner's China prerequisites (company, WeChat and
Alipay verification, SMS signature, ICP filing, PIPL policy, legal advice on the generative-AI
rules) in `docs/lm/accounts.md`.
