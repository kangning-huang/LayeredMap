# Accounts and Pro access (plan, Phase 7)

Owner decision, 2026-10-19: Pro needs an account. At first Pro is free for anyone who registers;
a subscription may come later. The app only needs to know who is who, so it can grant Pro.

This page is the plan. How it is built (endpoints, tokens, the redirect, the Ask proxy, the China
adapter): `docs/lm/accounts-design.md`.

A Mac App Store channel with Pro as an in-app purchase, which needs no account and no WeChat or
Alipay sign-in, is planned in `docs/lm/app-store.md`.

## Principles

- **Store as little as possible.** Per user: the sign-in service's user ID, how they signed in, an email or phone number when the provider gives one, `pro` (true/false), the sign-up date, and later the subscription status. No names, no avatars, no map data.
- **No user data in a git repository**, encrypted or not. A key the app can use to decrypt is a key anyone can extract; git history can't really be deleted (GDPR and PIPL give people the right to deletion); git can't handle two sign-ups at once; and one wrong visibility click publishes everything. Accounts live in a sign-in service and a database.
- **The OpenRouter key leaves the app.** Today the Pro test build carries the key, which is why those builds can't be shared. With accounts, the Ask box goes through a small server function that holds the key, checks the user's token and Pro flag, and enforces per-user limits so one account can't run up the bill.
- **Sign-in happens in the browser.** The app opens the system browser, the person signs in there, and the browser hands a one-time code back to the app (OAuth with PKCE, through the app's URL scheme or a loopback address, as RFC 8252 describes). The app keeps its refresh token in the macOS keychain or Windows Credential Manager. The app never sees a password.

## Two regions

Mainland China needs its own sign-in methods and, in practice, its own servers. Google, X and Reddit sign-in are blocked there; WeChat, Alipay and phone-number codes are what people use.

| | International | Mainland China (凌波微图) |
|---|---|---|
| Sign-in | Sign in with Apple, Google, X, email link | WeChat (微信登录), Alipay (支付宝登录), phone-number code (手机号验证码), Sign in with Apple |
| Service | Supabase (Auth + Postgres + Edge Functions), or Firebase | A China-hosted identity service with WeChat, Alipay and SMS built in, such as Authing, Tencent CloudBase or Alibaba Cloud's identity service; or our own small service on a China cloud |
| Assistant | Server function → OpenRouter | Server function → a model provider reachable and permitted in China (see below) |
| Payments later | Stripe, or Paddle / Lemon Squeezy (they handle sales tax as merchant of record) | WeChat Pay and Alipay (directly, or through Stripe, which supports both for some merchants) |

The app asks once which region the person is in (default from the Mac's region), then signs in
there. Accounts aren't merged across regions at first; a person who moves can register again.

Reddit login isn't built into Supabase, Firebase or the Chinese services, and isn't worth custom
work: Apple, Google, X and email cover nearly everyone outside China.

## What the China side needs (owner)

These are prerequisites, not code. Most need a company registered in mainland China.

1. **A mainland company entity** (or a partner who operates the service). WeChat Open Platform, Alipay Open Platform and SMS providers verify a business licence.
2. **WeChat Open Platform** (open.weixin.qq.com): developer verification (开发者资质认证, an annual fee), then a website or mobile app registration for 微信登录. A desktop app uses the website (QR code) flow in the browser.
3. **Alipay Open Platform** (open.alipay.com): an enterprise account and an app with the 获取会员信息 / 支付宝登录 capability.
4. **SMS codes**: an SMS provider in China (Alibaba Cloud, Tencent Cloud) with an approved signature (短信签名) and message template.
5. **ICP filing** (ICP备案) for the domain the China sign-in pages and server use, since they're hosted in mainland China.
6. **A Chinese privacy policy** under PIPL (个人信息保护法): what's collected (phone number or WeChat/Alipay ID), why, how long, and how to delete. If any China users' data is stored outside China, the cross-border transfer rules apply; keeping China accounts on China servers avoids that.
7. **The Ask box in China.** Offering a generative AI service to the public in mainland China falls under the Interim Measures for Generative AI Services, which require using a model that has completed filing (备案) and labelling AI output. OpenRouter and most overseas models are also unreachable from mainland networks. The China Pro tier would use a domestic model with completed filing (for example from Alibaba, Baidu, Zhipu, DeepSeek or Moonshot, through their own APIs) behind the same server function. This needs legal advice before launch.

## Work items (proposed for Phase 7)

1. Sign-in in the app: a Settings › Account pane and a sign-in prompt where Pro features start. The browser flow with PKCE, the token kept in the keychain, sign out, and delete account.
2. The international service: a Supabase project with Apple, Google, X and email; a `profiles` table (`id`, `provider`, `email`, `pro`, `created_at`, later `plan`, `renews_at`); row-level security so a user can read only their own row.
3. The Ask proxy: a server function that checks the token and `pro`, applies per-user daily limits, calls the model and streams the answer back. The OpenRouter key exists only there. The Pro test build's bundled key and its XOR pad are removed.
4. The China service: WeChat, Alipay and phone sign-in through a China identity service, the same `profiles` shape, and an Ask proxy with a filed domestic model. Blocked on items 1, 5 and 7 of the owner's list above.
5. Later, a subscription: a checkout page, a payment webhook that sets `plan` and `renews_at`, and the app reading `plan` instead of plain `pro`.

## Costs (check current pricing before choosing)

Supabase and Firebase both have free tiers covering tens of thousands of monthly active users,
more than a pilot needs. The China side's fixed costs are mostly the company, the WeChat developer
verification fee and SMS messages (a few fen each).
