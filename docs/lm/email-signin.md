# The sign-in email

**Decision, 2026-10-22 (owner):** sign-in emails go through **custom SMTP on Resend**, sender
`noreply@nimblemap.com`, name **NimbleMap**, with the repo's templates
(`services/ask-proxy/supabase/templates/`, subject "Your NimbleMap sign-in code" for both Magic
link and Confirm signup). The Supabase side was configured by the coordinator with the owner's
OK; nothing here changes the live project by itself.

## What people get

The app asks `POST /auth/v1/otp` with `create_user: true` and a PKCE challenge
(`lm_account::Account::begin`). Supabase sends **Confirm signup** to an address it doesn't know
yet and **Magic link** to a known one; with the repo's templates both look the same:

- from NimbleMap (`noreply@nimblemap.com`), subject "Your NimbleMap sign-in code";
- the 6-digit code (`{{ .Token }}`), large, good for an hour (`otp_expiry = 3600`,
  `otp_length = 6`, matching `lm_account::code::LEN`);
- the link (`{{ .ConfirmationURL }}`), which signs in on the computer that asked within the app's
  10-minute wait;
- a line that the email can be ignored by someone who didn't ask for it.

Either signs in; whichever comes first wins (`docs/lm/accounts-design.md` › Email: the link or the
code). The link goes to `/auth/v1/verify`, which for a PKCE token (`pkce_…`) redirects to the
app's `http://127.0.0.1:<port>/callback?state=…` with `?code=`; that needs
`http://127.0.0.1:*/**` in the project's Redirect URLs (Authentication › URL Configuration;
`config.toml` has it), or Supabase falls back to the Site URL and the app keeps waiting.

Settings › Account says what to expect:

- button: "Email me a sign-in code";
- before sending: "We'll email you a 6-digit code from NimbleMap.";
- after: "We sent a code to {address}. Type it below, or click the link in the same email on this
  Mac." and "The email comes from NimbleMap, subject “Your NimbleMap sign-in code”. If it isn't
  in your inbox, check your spam or junk folder.";
- after the link's 10 minutes: "The link has stopped working on this Mac. Type the code from the
  email, or send a new one."

If the subject or sender in the templates or the SMTP settings change, change these strings (and
the 10 catalogs) with them.

## Before: Supabase's built-in mailer

Until 2026-10-22 the project used Supabase's built-in mailer, whose default templates can't be
edited there. The owner received:

| Who | Sender | Subject | Body |
|---|---|---|---|
| A new address | Supabase Auth | Confirm your email address | "Follow the link below to confirm this email address and finish signing up." + **Confirm email address** |
| A known address | Supabase Auth | Your sign-in link | "Follow the link below to sign in. This link expires shortly and can only be used once." + **Sign in** |

(Defaults from `supabase/auth`, `internal/mailer/templatemailer/templatemailer.go`; older
projects may say "Your Magic Link".) Neither named NimbleMap or carried a code (no
`{{ .Token }}`), and the built-in mailer delivers only to the project team's addresses, at most
2 emails an hour ([Supabase: custom SMTP](https://supabase.com/docs/guides/auth/auth-smtp));
anyone else got "Email address not authorized". So real users needed custom SMTP regardless.

## Options that were weighed

| Option | Cost (at the time; check the provider's page) | Setup | Notes |
|---|---|---|---|
| **Resend** (chosen) | Free: 3,000 emails/month, 100/day, one domain; Pro $20/month for 50,000 | Domain in Resend, SPF/DKIM (and DMARC) records in DNS, an SMTP key; Supabase › Authentication › Emails › SMTP Settings: `smtp.resend.com`, port 465, user `resend` | Simplest; Supabase's own guide uses it |
| Postmark | Free: 100/month (testing); about $15/month for 10,000 | The same, with a server token | Strong deliverability; no useful free tier |
| Amazon SES | About $0.10 per 1,000 | AWS account, domain verification, leaving the SES sandbox (reviewed by AWS) | Cheapest at volume, most setup |
| Brevo / ZeptoMail | Brevo free: 300/day; ZeptoMail per credit | As Resend | Alternatives |
| Built-in mailer | Free | None | Team addresses only, 2 an hour, Supabase-branded |

## Checklist after the switch

1. Templates and settings pushed (`services/ask-proxy/README.md`, step 6) or pasted in the
   Dashboard.
2. Authentication › Rate Limits › emails per hour raised from 30 to what the Resend plan allows.
3. A sign-in sent to a new and to a known address: both show the code and the link, from
   NimbleMap, and the code and the link each sign in.
4. Privacy: Resend receives the address to deliver the email (`web/lm/privacy.html` says so).
