# In-app feedback (Help › Send Feedback…)

Owner decision, 2026-10-20: people send feedback from the app, and it appears as a GitHub
Discussion on the public repository `kangning-huang/NimbleMap`. The app posts it through a
NimbleMap server function, so nobody needs a GitHub account and it works where GitHub is hard to
reach (mainland China). Anyone may send, signed in or not, and spam is held back with limits.

Owner decision, 2026-10-20 (later the same day): a message from a signed-in user carries that
user's account id, **privately**. The app sends the session's access token; the server verifies it
and keeps the id in a private table beside the discussion. The public post says only "From a
signed-in user" (see [Signed-in senders](#signed-in-senders)).

Code: the sheet `crates/lm-ui/src/feedback_ui.rs`; the request, checks and install id
`crates/lm-account/src/feedback.rs`; the server function `services/ask-proxy/supabase/functions/feedback/`
with its logic in `_shared/feedback.ts`; the tables in
`services/ask-proxy/supabase/migrations/20261020000000_feedback.sql` and
`20261020120000_feedback_senders.sql`. Setup and deploy:
`services/ask-proxy/README.md` › Feedback.

## The sheet

Help › Send Feedback… (both macOS and Windows) opens a sheet over the map:

- a segmented choice, **Idea · Problem · Other**;
- **Message** (required, 5 to 4,000 characters), **Title (optional)**, **Email (optional)** with
  "Only used to reply to you. Never shown publicly.";
- **Include app version and macOS version** (Windows: "… and Windows version"), on by default;
- "Your feedback will be posted publicly on GitHub Discussions, without your email.";
- signed in (to the international service), under it: "Sent with your account so we can follow
  up. Your account isn't shown publicly.";
- **Cancel** and **Send**. Send shows "Sending…" with a spinner and locks the form; success shows
  the discussion's address with **View on GitHub** (opens the browser) and **Done**; a failure
  keeps the text and says what to do (wait until tomorrow, check the connection, turn off Work
  Offline, shorten the message).

Keys: ⌘↩ sends, Esc cancels (the sheet takes both before the shell's shortcuts, so Esc doesn't
also clear the selection behind it). A click outside does nothing, so typing isn't lost. Every
control has an accessible name; "Sending…", the success line and errors are announced.

The draft is checked in the app with the server's bounds (length, at most 3 links, a plausible
email), so most problems are shown before anything is sent. Strict offline mode refuses the
request (network purpose `Feedback`). Builds without an account config (`account.json`) can't send
and say so, with the Discussions address to post by hand.

There is no Settings link: Settings holds only the Account pane, and feedback isn't about the
account. Help is where Mac apps put it.

## What is sent, posted and kept

| | Sent by the app | Posted on GitHub (public) | Kept on the server (private) |
|---|---|---|---|
| Message, title, kind | yes | yes (quoted, see below) | no |
| App version, system version | only when the box is ticked | same | no |
| Interface language (`de`, `zh`) | yes | yes | no |
| Reply email | only when typed | **never** | `feedback_contacts`, with the discussion's address |
| Install id (random UUID) | yes | never | only as a keyed hash in `feedback_rate`, for a week |
| Network address | (seen by the server) | never | only as a keyed hash in `feedback_rate`, for a week |
| Account id | the session's access token, only when signed in | **never** (only "From a signed-in user") | `feedback_senders`, with the discussion's address, when the token verifies |
| Email of the account, maps, files | never | never | never |

- **Install id:** a random UUID made on first send and kept in the support folder
  (`~/Library/Application Support/LayeredMap/feedback-install-id`; `%APPDATA%\LayeredMap\` on
  Windows). It is not the account and says nothing about the person; deleting the file makes a new
  one.
- **Hashes:** HMAC-SHA-256 under `FEEDBACK_HASH_KEY` (the service role key when unset), so the ids
  and addresses can't be recovered by trying them all.
- **Reply emails:** row level security on, no policies, nothing granted to `anon` or
  `authenticated`: only the service role (the function and the dashboard) reads them. Delete a row
  once the conversation is over; at most a year (`README.md` › Housekeeping).
- **Logs:** one line per message: kind, status, whether an email was kept, `auth=user|none|invalid`
  and whether a sender row was kept. Never the text, an address, a hash, a token or an id.

## What the discussion looks like

Category by kind: Idea → **Ideas**, Problem → **Q&A** (an answer can be marked, which fits
"is this fixed?") titled `[Problem] …`, Other → **General**. The ids are function env vars
(`FEEDBACK_CATEGORY_IDEA`, `_PROBLEM`, `_OTHER`), defaulting to this repository's categories.

The title is the user's, else the message's first line shortened at a word to about 80
characters. The body:

```markdown
> The time slider skips March when I press →.
> It happens with the storms example.

---

NimbleMap 0.7.3 · macOS 14.5 · de

Sent from the NimbleMap app
```

From a signed-in sender (verified token), one more line, and nothing else:

```markdown
…

Sent from the NimbleMap app

From a signed-in user
```

The details line is the app's version as "NimbleMap 0.7.3", the system as the app sends it
(`About::current`: "macOS 14.5" from `sw_vers -productVersion`, "Windows" on Windows), and the
interface language; the versions only when the box is ticked. A bare system version (an older
build, or a hand-made request: "26.6") is labelled when posted, "macOS 26.6", or "Windows
10.0.26100" for a Windows build number (four or more digits in the third part); a version that
already names its system is left as it is, so nothing is labelled twice.

The user's text can't ping, link or embed: every line is quoted; `\ ` * _ [ ] < > ! # | ~ &` are
backslash-escaped (no images, links, HTML, entities, headings or tables); a zero-width space
follows every `@` and every `#` before a digit (no mentions, no issue cross-references). Control
characters and bidirectional overrides are stripped. Bare URLs still show as text links.

## Signed-in senders

- **App** (`lm_account::Account::feedback_token`, `feedback::send`): when someone is signed in to
  the service feedback goes to (the international one), the request adds
  `Authorization: Bearer <access token>`, refreshed first as for Ask. Signed out, or a token that
  can't be had (offline, a refused refresh), and the message goes without it, as anyone's. A
  mainland China session is another service, so its feedback goes without the account. The body
  never carries an id: the server wouldn't trust one.
- **Server** (`feedbackSender` in `_shared/feedback.ts`): the token is checked with
  `auth.getClaims` (the project's signing keys, as Ask does) and the claims must be a signed-in
  user's, unexpired (`_shared/auth.ts`). Verified: `sub` goes into `feedback_senders` after the
  discussion is created, the same way as a reply email, and the post gets "From a signed-in user".
  Anything else (expired, forged, an anon key, a verifier failure) doesn't stop the message: it is
  posted as anonymous and logged `auth=invalid`, never the token.
- **Limits:** the same as anyone's (per install, per address, overall); signing in doesn't raise
  them.
- **The table:** `feedback_senders (discussion_url text primary key, user_id uuid not null
  references auth.users on delete cascade, created_at timestamptz default now())`, row level
  security on, no policies, nothing granted to `anon` or `authenticated`. Deleting an account
  deletes its rows. Delete rows once a conversation is over; at most a year.
- **Looking up a sender** (owner; Supabase Dashboard › SQL Editor, which runs as the service role):

  ```sql
  select s.discussion_url, s.user_id, u.email, s.created_at
    from public.feedback_senders s
    join auth.users u on u.id = s.user_id
   where s.discussion_url = 'https://github.com/kangning-huang/NimbleMap/discussions/42';
  ```

  Leave out the `where` line (and add `order by s.created_at desc`) to list them all.

## Abuse controls

- **Per install:** 5 messages per UTC day (`FEEDBACK_DEVICE_DAILY`).
- **Per network address:** 20 per day (`FEEDBACK_IP_DAILY`): a school or office shares one.
- **Overall:** 200 per day (`FEEDBACK_GLOBAL_DAILY`): a ceiling on what someone with many addresses
  can post.
- **Bounds:** body ≤ 32 KB, message 5–4,000 characters, title ≤ 120, at most 3 links, a plausible
  email.
- **Honeypot:** a `website` field the app never fills; a request with it is told it worked and
  nothing is posted or counted.
- The limits are counted before posting (atomically, `feedback_reserve`), so a failed post still
  counts: a retry storm can't get past them.
- The GitHub token can do one thing: read and write Discussions on this repository.

Errors the app maps to its own words: `rate_limited` (429), `too_long` (413), `invalid` (400, with a
short English reason), `upstream` (502/503: GitHub or the database failed, or the function isn't
set up).

## Mainland China

The app talks only to the feedback function (`https://<ref>.supabase.co/functions/v1/feedback`);
the function talks to GitHub. Nobody's computer needs to reach GitHub. Supabase's hosted domains
are generally reachable from mainland China but can be slow or blocked by some networks; if that
becomes a problem, a China-hosted relay can forward the same JSON to the function (an optional
`feedback_url` in `account.json` would point the app at it; not built).
