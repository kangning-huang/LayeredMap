# The sign-in email: what people get today, and how to brand it

Status, 2026-10-22: nothing here is deployed. Every change to the live Supabase project (SMTP,
templates, rate limits, redirect URLs) needs the owner's OK first.

## What people get today

The hosted project still uses Supabase's built-in mailer and its default templates (the ones in
`services/ask-proxy/supabase/templates/` were never pushed). The app asks
`POST /auth/v1/otp` with `create_user: true` and a PKCE challenge (`lm_account::Account::begin`),
so Supabase sends one of two emails:

| Who | Template | Sender | Subject | Body |
|---|---|---|---|---|
| A new address | Confirm signup | Supabase Auth | Confirm your email address | "Follow the link below to confirm this email address and finish signing up." + **Confirm email address** |
| A known address | Magic link | Supabase Auth | Your sign-in link | "Follow the link below to sign in. This link expires shortly and can only be used once." + **Sign in** |

(Defaults from `supabase/auth`, `internal/mailer/templatemailer/templatemailer.go`; projects
created before the 2025 template refresh may still say "Your Magic Link".) Neither email names
NimbleMap, and **neither contains a 6-digit code**: the defaults leave out `{{ .Token }}`. The
app's code field therefore has nothing to take until the templates change.

The link does sign in, on the same computer within the app's 10-minute wait: both links go to
`/auth/v1/verify`, which, for a PKCE token (`pkce_…`), redirects to `redirect_to` (the app's
`http://127.0.0.1:<port>/callback?state=…`) with `?code=`, and the app exchanges it as for Google
or Apple. That needs `http://127.0.0.1:*/**` in the project's Redirect URLs (Authentication ›
URL Configuration; `config.toml` has it); without it Supabase falls back to the Site URL and the
app keeps waiting.

Two limits of the built-in mailer matter more than the wording
([Supabase: custom SMTP](https://supabase.com/docs/guides/auth/auth-smtp)):

- it delivers only to addresses on the project's organization team; anyone else gets
  "Email address not authorized" (the app shows it as a server error), and
- it sends at most 2 emails an hour, and has no delivery guarantee.

So email sign-in works today only for the owner's own team addresses. Real users need custom
SMTP whatever the wording.

With branch `email-signin` the app says what to look for (Settings › Account): before sending, "We'll email you
a link from Supabase Auth, the service that runs NimbleMap accounts. Open it on this Mac."; after,
the sender, both subjects, the spam folder, and "If the email shows a 6-digit code, you can type
it here instead." The button reads "Email me a sign-in link".

## The repo's templates

`services/ask-proxy/supabase/templates/magic_link.html` and `confirmation.html` already do what
the app was designed for: both show `{{ .Token }}` (the 6-digit code, large) and
`{{ .ConfirmationURL }}` (the link), say NimbleMap, and have the subject "Your NimbleMap sign-in
code". `config.toml` sets `otp_length = 6` and `otp_expiry = 3600` to match the app
(`lm_account::code::LEN`). They take effect only with custom SMTP: Supabase ties template edits
to it on hosted projects (the owner found the built-in mailer's templates can't be changed).

## Options

| Option | Cost (check before signing up) | What the owner does | Result |
|---|---|---|---|
| **A. Resend** (recommended) | Free: 3,000 emails/month, 100/day, one domain; Pro $20/month for 50,000 | Sign up; add `nimblemap.com` (or a subdomain such as `mail.nimblemap.com`) and put its SPF/DKIM records (plus a DMARC record) in DNS; create an SMTP key; in Supabase › Authentication › Emails › SMTP Settings enter host `smtp.resend.com`, port 465, user `resend`, the key, sender `no-reply@nimblemap.com`, name "NimbleMap" | Sender "NimbleMap", our subjects and the code. Simplest setup, Supabase's own guide uses it |
| B. Postmark | Free: 100 emails/month (testing); about $15/month for 10,000 | The same: domain, DNS records, a server token as the SMTP user and password | Best known deliverability for sign-in mail; no useful free tier |
| C. Amazon SES | About $0.10 per 1,000 emails | An AWS account, domain verification, and a request to leave the SES sandbox (a day or two, reviewed by AWS) | Cheapest at volume, most setup |
| D. Brevo / ZeptoMail | Brevo free: 300/day; ZeptoMail pay per credit | As A | Alternatives if Resend doesn't fit |
| E. Stay on the built-in mailer | Free | Nothing | Team addresses only, 2 an hour, Supabase-branded: not usable for real users |

After any of A–D:

1. Push the templates and settings (`services/ask-proxy/README.md`, step 6: `supabase config push`,
   reading the Auth diff, or paste the two templates in the Dashboard).
2. Raise Authentication › Rate Limits › emails per hour (custom SMTP starts at 30) to what the
   provider's plan allows.
3. Send a sign-in to a new and a known address; both should show the code and the link.
4. Change the app's text back: the sender becomes "NimbleMap" and the subject "Your NimbleMap
   sign-in code", so the lines naming "Supabase Auth" and the default subjects in
   `crates/lm-ui/src/account_ui.rs` (and the 10 catalogs) go, and the button can read "Email me a
   sign-in code" again; `web/lm/privacy.html` names the email service.
5. Privacy: the email provider becomes a processor of the address (add it to
   `web/lm/privacy.html`).

Recommendation: A (Resend) on a `nimblemap.com` sender. It costs nothing at today's volume, takes
about half an hour plus DNS propagation, and turns on the code that the app and templates already
support. What the owner needs to decide: the provider, the sender address and name, and access to
the domain's DNS.
