# Signing and notarization (WP0.1)

macOS blocks unsigned apps downloaded from the internet, so every gate build is signed with a
Developer ID and notarized. This needs an Apple Developer Program membership (USD 99 a year) —
an owner action, planned for week 1 of Phase 0.

Once enrolled, add these repository secrets (Settings › Secrets and variables › Actions):

| Secret | What it is |
|---|---|
| `LM_DEVELOPER_ID_P12_BASE64` | The "Developer ID Application" certificate and key, exported as .p12, base64-encoded (`base64 -i cert.p12 \| pbcopy`) |
| `LM_DEVELOPER_ID_P12_PASSWORD` | The .p12 export password |
| `LM_SIGN_IDENTITY` | e.g. `Developer ID Application: Kangning Huang (TEAMID1234)` |
| `LM_NOTARY_APPLE_ID` | The Apple ID email used for notarization |
| `LM_NOTARY_TEAM_ID` | The 10-character team ID |
| `LM_NOTARY_PASSWORD` | An app-specific password from appleid.apple.com |

`.github/workflows/lm-ci.yml` then signs with the hardened runtime, submits with `notarytool`,
staples the ticket and checks it with `stapler validate` and `spctl`. Without the secrets the app
is ad-hoc signed: it runs on the machine that built it; elsewhere macOS warns.

Locally: `packaging/lm/bundle-macos.sh` (same environment variables).

## The installer disk image (WP5.7)

Students install from a .dmg: the app beside a link to Applications, so installing is one drag.

```sh
packaging/lm/build-dmg.sh [out-dir]                 # LayeredMap-<version>-<build>-macos-arm64.dmg
LM_TIER=pro packaging/lm/build-dmg.sh [out-dir]     # LayeredMap-Pro-…
```

The script runs `bundle-macos.sh` first (so the app inside is signed, notarized and stapled as
above), then builds the image with `hdiutil` (HFS+, compressed, verified). With the same
environment variables it then signs the .dmg, submits it to `notarytool`, staples the ticket,
and runs the two Gate 5 checks:

```sh
xcrun stapler validate LayeredMap-….dmg
spctl --assess --type open --context context:primary-signature --verbose=2 LayeredMap-….dmg
```

Without the variables the .dmg is unsigned and the app inside ad-hoc signed: fine for testing on
the building Mac, but another Mac shows a security warning. The CI `app` job builds the Free
.dmg on every run and uploads it with the .zip.

Signing checks added in Phase 5: `bundle-macos.sh` fails when a signed app lacks the hardened
runtime flag, and the bundle carries `LICENSE`, `ATTRIBUTION.md` and `ATTRIBUTION-crates.md` in
`Contents/Resources`. Pro test builds still embed the owner's OpenRouter key from the building
Mac's keychain, as before (`LM_NO_KEY=1` leaves it out); the .dmg step changes nothing about that,
so never upload a Pro test .dmg to a public host.

**Download host (owner decision).** The spec ships the .dmg from a China-reachable host, not from
GitHub releases. The host is not chosen yet, so its address is configuration, not code:
`LM_DOWNLOAD_BASE` (where the .dmg files are uploaded; the CI reads the repository variable of
that name) and, in the app, `LM_UPDATE_FEED_URL` at build time (`docs/lm/update-feed.md`). With
`LM_DOWNLOAD_BASE` set, `build-dmg.sh` also writes `update-feed.json` for the host.
