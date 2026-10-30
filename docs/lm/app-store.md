# The Mac App Store and in-app purchase for Pro (plan)

*Proposed 2026-10-19. Packaging scaffolding started 2026-10-30 (`LM_CHANNEL=mas` in
`packaging/lm/bundle-macos.sh`, `app-mas.entitlements`, `PrivacyInfo.xcprivacy`,
`container-migration.plist`). Still needs Apple Distribution + Mac Installer Distribution
certificates, a Mac App Store provisioning profile, and App Store Connect bundle ID alignment
(`app.layeredmap.desktop` vs the Connect record).*

**Owner decision, 2026-10-19: Pro stays free with an account for now.** The first goal is to make
NimbleMap good and popular; monetizing comes later, if the user base grows. This page is the plan
for that later step, so it's ready when needed. Until then the direct download with Supabase
sign-in is the only channel for paying users; the Mac App Store free listing can ship without IAP.

The owner asked whether NimbleMap can be sold through the Mac App Store, with Pro as an in-app
purchase, so that people in mainland China can buy Pro without WeChat, Alipay or phone sign-in.
This page is the plan. It changes `accounts.md`: App Store buyers need no account at all.

## Summary

- **Two channels.** The Mac App Store build (sandboxed, Pro as an in-app purchase, no sign-in),
  and the direct download from the website (Developer ID, Supabase sign-in, as built on the
  `accounts` branch). Windows stays on the direct channel.
- **Apple handles payment everywhere, China included.** Chinese buyers pay Apple through their
  Apple ID, usually with Alipay or WeChat Pay. No Chinese company is needed to be paid, and no
  WeChat or Alipay integration is needed.
- **China doesn't become free of licensing.** The China storefront needs an ICP filing number for
  any app, and an AI assistant needs a government licence. The realistic first step in China is
  the free app; Pro there waits for a filed domestic model and the licence.

## What the App Store changes

| | Direct download | Mac App Store |
|---|---|---|
| Install and updates | .dmg; the app's update check (`lm-ui/src/update.rs`) | The App Store installs and updates; no update check in this build |
| Signing | Developer ID Application, notarized | Apple Distribution, plus Mac Installer Distribution for the .pkg; Apple signs the download |
| Sandbox | No | Required (App Review 2.4.5) |
| Who has Pro | A Supabase account with `pro` | A verified App Store purchase |
| Ask box server check | Supabase access token | Apple's signed transaction |
| Payment | Later: Stripe or a merchant of record | In-app purchase; Apple keeps 15 % (Small Business Program, under USD 1 million a year) or 30 % |
| China storefront | Not applicable (download from the website) | Needs an ICP filing number; Pro needs the AI licence |

## Pricing, when the time comes

Owner's choice (2026-10-19): a subscription with a free trial, kept very cheap.

| | |
|---|---|
| Monthly | US$0.99 (Apple sets the local prices, e.g. ¥6 in China, €0.99 in the eurozone; each can be adjusted) |
| Free trial | 1 month, as an introductory offer (Apple allows 3 days to 1 year; once per Apple ID) |
| Yearly | US$9.99, offered beside the monthly price: far less churn, and the money up front |
| Allowance | 300 questions a month, plus a short-burst limit; the Ask box says plainly when it's used up and when it resets |

**What a subscriber brings in.** US$0.99, less Apple's 15 % (Small Business Program; also 15 % for
any subscriber in their second year), less sales tax or VAT where the price includes it (EU, UK,
China): about US$0.70–0.84 a month. **What a subscriber costs.** The default model costs
US$0.0008 per question, or US$0.0016 with a 2× margin for bigger maps and retries
(`llm-eval.md` › Cost per active user): US$0.03 at 20 questions a month, US$0.16 at 100, US$0.48
at the 300 allowance. A subscriber stays profitable up to about 400 questions a month.

**Fixed costs.** Supabase Pro (US$25 a month, needed once real users rely on it: the free plan
pauses an idle project), the Apple membership (US$99 a year), a domain and a mail sender: about
US$35 a month, so roughly 45–50 subscribers to break even.

**Not monthly on the direct download.** Card fees take about 30 % of a US$1 charge (Stripe's
2.9 % + US$0.30), and merchants of record take more on small amounts. On the direct channel,
offer the yearly price only, or keep Pro free with an account.

**Until then (free Pro).** Each account has a monthly allowance of about 300 questions (owner,
2026-10-19): 1,000 model calls and 3 million tokens per UTC month (`ASK_MONTHLY_REQUESTS`,
`ASK_MONTHLY_TOKENS`; one question is usually 2–4 calls), at most about US$0.24–0.48 a month per
account. The daily allowance (60 calls, about 20 questions) stays as a burst guard. When the month's
allowance is used up, the Ask box says so and when it renews.

## Pro as an in-app purchase

- **Product.** One non-consumable "NimbleMap Pro" or an auto-renewing subscription. The Ask box
  has a running cost per question (the model), so a subscription with a free trial fits better
  than a one-off price; a one-off purchase would need a firm daily allowance forever. Owner's
  choice; the server enforces the daily allowance either way.
- **"Free at first" can't be an in-app purchase.** Apple has no free in-app purchases. Either give
  Pro to everyone without a purchase at launch (the app simply doesn't ask), or offer a free trial
  on the subscription. A free trial is the usual way and still counts the buyer as a customer.
- **In the app.** StoreKit 2 is a Swift API, so purchases go through a small Swift helper, in the
  same way as the voice helper (`packaging/lm/dictate.swift`): list the product and its price,
  buy, restore, and hand back the signed transaction (`Transaction.jwsRepresentation`) and the
  signed app transaction. The Rust side stays free of `unsafe` code.
- **On the server.** The Ask proxy (`services/ask-proxy`) gets a second way in: instead of a
  Supabase token, the app sends the signed transaction. The function checks the signature chain
  up to Apple's root certificate, the bundle ID, the product ID, the environment (sandbox or
  production) and that the purchase hasn't been refunded or revoked, then applies the daily
  allowance per `originalTransactionId`. App Store Server Notifications (version 2) tell the server
  about refunds, renewals and lapses. Nothing about the buyer except that ID is stored.
- **The same entitlement.** The direct download already unlocks Pro from one answer, the account's
  `Entitlement` (`pro`, `plan`, `expires_at`; `accounts-design.md` › Entitlement). A verified
  purchase feeds that same `Entitlement` (the plan is the product, `expires_at` the subscription's
  end), so the interface's locked and unlocked states need no change.
- **Accounts become optional.** An App Store buyer never signs in. Sign in with Apple can come
  later for anyone who wants Pro on two Macs with different Apple IDs, or on Windows.
- **Review rules this already meets.** In-app purchase for digital features (3.1.1): the App Store
  build never links to another way to pay. Sign in with Apple wherever Google or X sign-in is
  offered (4.8): the App Store build offers no sign-in at first. Account deletion in the app
  (5.1.1(v)): built in Settings › Account for the direct channel.

## The sandbox work

The App Store build runs in the App Sandbox. What that touches in NimbleMap:

1. **Entitlements.** `app-sandbox`, `files.user-selected.read-write` (open and save panels,
   drag and drop, Open With), `files.bookmarks.app-scope` (recent files), `network.client`
   (basemap tiles, the data library, the Ask box) and, for Pro, `device.audio-input` (the voice
   helper, which also needs the sandbox and `inherit` entitlements in its own signature).
2. **Shapefiles and other multi-file formats.** The sandbox grants access only to the file the
   person opened, not the `.dbf`, `.shx`, `.prj` and `.cpg` beside a `.shp`. macOS allows this
   through related items: the Info.plist declares those extensions with `NSIsRelatedItemType`,
   and the app asks for each sibling through file coordination. Same for a GeoTIFF's `.aux.xml`
   and `.tfw`, and a CSV's `.prj`. This is the biggest risk and needs a spike first.
3. **Command-line tools.** The app runs `/usr/bin/security` (the keychain: `lm-ask`, `lm-account`),
   `/usr/bin/defaults` (the region and languages), `/usr/bin/open` (the browser) and `sw_vers`.
   A sandboxed app has to use the system frameworks instead: Security.framework for the keychain,
   the locale APIs for region and languages, NSWorkspace to open a link, and the operating system
   version from the process. Done through existing crates (`security-framework`, `objc2`), so
   NimbleMap's own crates stay free of `unsafe`; this also improves the direct build.
4. **Data folders.** Settings, autosave, logs and caches move into the app's container
   (`~/Library/Containers/<bundle id>/Data/Library/...`). A `container-migration.plist` in the
   bundle tells macOS to move the old `~/Library/Application Support/LayeredMap` files in on first
   launch, so people coming from the direct download keep their settings. `lm_core::platform`
   already resolves every folder from `HOME`, which points into the container when sandboxed.
5. **Recent files and reopening maps.** A saved `.lmap` that refers to its source files needs
   security-scoped bookmarks to reopen them later; map packages carry their data inside and are
   unaffected.
6. **The update check and Update Now** are compiled out of the App Store build
   (`LM_UPDATE_FEED_URL=none LM_UPDATE_FEED_FALLBACK_URL=none`; docs/lm/update-feed.md): the App
   Store updates the app, and a sandboxed app can't replace itself.
7. **A privacy manifest** (`PrivacyInfo.xcprivacy`) listing the "required reason" APIs used (file
   timestamps, user defaults, disk space) and the data collected (none, or for Pro the purchase
   ID). The App Store privacy label says the same.

## The build

A second flavour of `packaging/lm/bundle-macos.sh`, chosen with `LM_CHANNEL=mas`:

- the sandbox entitlements, the embedded provisioning profile, and signing with the Apple
  Distribution certificate (the voice and purchase helpers too);
- `productbuild` to a signed `.pkg` with the Mac Installer Distribution certificate;
- upload with Transporter or `xcrun altool`, then TestFlight for Mac before review;
- no bundled OpenRouter key, ever (App Store builds are public).

CI can build the `.pkg` once the two certificates and the profile are repository secrets, as for
the Developer ID build (`signing.md`).

## China

1. **ICP filing for the app.** Apple has required an ICP filing number for apps in the China
   storefront since September 2023. Reports say it needs a company in China or a local publisher
   partner; whether an individual with a Chinese ID can file an app like this is not confirmed.
   Check with a filing agent, or through Alibaba Cloud's or Tencent Cloud's filing service.
2. **The free app first.** With the filing number, the free app can go into the China storefront.
3. **Pro in China waits for the AI licence.** In 2023 Apple removed more than 100 generative AI
   apps from the China storefront and told developers they needed a licence from the Ministry of
   Industry and Information Technology. Until NimbleMap has a filed domestic model and that
   licence, the Pro purchase isn't offered in the China storefront: the product's availability in
   App Store Connect leaves China out, and the app hides the Pro offer when the storefront
   (`Storefront.countryCode`) is CHN.
4. **What this replaces.** The WeChat, Alipay and SMS sign-in in `accounts.md` is no longer needed
   for App Store buyers. It would only matter for Pro on the direct download in China.

## Order of work

| Step | What | Waits on |
|---|---|---|
| 1 | Spike: Shapefile siblings and recent files under the sandbox, in a minimal sandboxed build | Membership (profile) |
| 2 | Replace the command-line tools with framework calls (helps both channels) | — |
| 3 | `LM_CHANNEL=mas` build, container migration, privacy manifest, update check off — **scaffolding in tree 2026-10-30**; signed upload waits on Distribution certs + profile | Step 1 (sandbox sibling spike still open) |
| 4 | StoreKit helper and the purchase screens | Owner: the product and price |
| 5 | Ask proxy: verify Apple transactions, server notifications | Owner: App Store Connect setup |
| 6 | TestFlight for Mac, then review (all storefronts except China) | Steps 3–5 |
| 7 | China storefront, free app only | ICP filing number |
| 8 | Pro in China | Filed domestic model and AI licence |

## The owner's part

1. Activate the Apple Developer membership (pending; the same problem blocks Developer ID signing).
2. In App Store Connect: create the app record (bundle ID `app.layeredmap.desktop`, the one app's
   id since Free and Pro merged on 2026-10-19, or a new ID for the App Store build; decide before
   the first upload, since it can't change), the Pro product,
   its price and storefronts, the Paid Applications agreement, and tax and banking details.
3. Join the App Store Small Business Program (15 % commission).
4. Create the certificates: Apple Distribution and Mac Installer Distribution, and a Mac App
   Store provisioning profile. Create an App Store Connect API key for uploads and for the server
   (In-App Purchase key) to check transactions.
5. China: ask a filing agent whether you can file the app as an individual, and what the AI
   licence would take.

## Open questions

- One-off purchase or subscription with a trial, and the price.
- One bundle ID for both channels, or a separate one for the App Store build. One ID makes the
  two installs the same app to macOS; separate IDs let them sit side by side.
- Whether Sign in with Apple should link an App Store purchase to the direct and Windows builds.
