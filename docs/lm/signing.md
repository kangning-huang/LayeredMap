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
