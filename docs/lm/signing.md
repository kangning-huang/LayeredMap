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

## Windows (WP6.2)

Windows doesn't block unsigned apps, but Microsoft Defender SmartScreen shows "Windows protected
your PC" for a download it has no reputation for, and students will stop there. A signature
names the publisher and lets reputation build up; **no certificate removes the warning on day
one any more**. EV certificates used to skip SmartScreen; Microsoft ended that, so OV and EV now
both earn reputation by downloads over days to weeks. Budget for that time: sign the beta from
its first public build, keep the same certificate (reputation follows it), and expect early
students to see the warning. The quickstart should show the "More info › Run anyway" screens.

Since June 2023 every publicly trusted code-signing key must live in certified hardware (a USB
token, a cloud HSM or your own HSM); a downloadable .pfx is no longer issued. Since March 2026 a
certificate lasts at most 460 days, so each is renewed yearly.

### Options (prices October 2026; **check before buying**)

| Option | What it costs | Fits LayeredMap? |
|---|---|---|
| **Azure Artifact Signing** (Microsoft's managed signing, formerly Trusted Signing) | Basic USD 9.99 a month for 5,000 signatures | **Recommended if eligible.** No token, signs in CI, keys held by Microsoft, certificates renewed for you. Public signing is open to organisations in the US, Canada, the EU and the UK and to **individual developers in the US and Canada**; identity is verified first (days). The publisher shown is the verified name. |
| OV certificate on a cloud HSM (SSL.com eSigner, DigiCert KeyLocker, GlobalSign) | Roughly USD 130–700 a year, plus the cloud-signing plan | Works anywhere, including a company outside those countries; CI signs through the vendor's tool. |
| OV certificate on a USB token (Sectigo, GlobalSign, DigiCert, SSL.com) | Roughly USD 130–700 a year, plus a token (USD 50–150) | Signing only on the machine with the token: no CI signing. |
| EV certificate | Roughly USD 350–1,100 a year | No SmartScreen advantage now; only for kernel drivers or a buyer who insists. Not needed. |
| Microsoft Store (MSIX) | Free or a one-time fee for the developer account (check) | The Store signs the package and SmartScreen doesn't apply; a later option, needs an MSIX package and Store review, and the Store isn't how students in China get software. |

### Setting up signing in CI

The installer script signs when `LM_SIGNTOOL_ARGS` holds signtool's arguments after `sign`
(`packaging/lm/windows/build-installer.ps1`): the exe, the installer and its uninstaller are
signed with a SHA-256 RFC 3161 timestamp, then checked with `signtool verify /pa`. Nothing secret
is in the repository.

For Azure Artifact Signing, once the account, identity validation and certificate profile exist,
add these repository secrets; the `windows-installer` job then signs:

| Secret | What it is |
|---|---|
| `LM_ARTIFACT_SIGNING_ENDPOINT` | The account's region endpoint, e.g. `https://eus.codesigning.azure.net` |
| `LM_ARTIFACT_SIGNING_ACCOUNT` | The signing account's name |
| `LM_ARTIFACT_SIGNING_PROFILE` | The certificate profile's name |
| `LM_AZURE_TENANT_ID`, `LM_AZURE_CLIENT_ID`, `LM_AZURE_CLIENT_SECRET` | An app registration with the account's certificate-profile signer role ("Trusted Signing Certificate Profile Signer" before the rename; check the current name) |

The job installs Microsoft's signing client (the `Azure.CodeSigning.Dlib.dll` signtool plug-in)
and points `LM_SIGNTOOL_ARGS` at it. Check the client package's current name and the timestamp
address in Microsoft's documentation when setting this up; the job is a stub until then.

For a certificate on a cloud HSM, install the vendor's signing tool or key storage provider in
the job instead and set `LM_SIGNTOOL_ARGS` to, e.g., `/fd SHA256 /tr <vendor timestamp URL> /td
SHA256 /sha1 <certificate thumbprint>`.

Without any of these the installer is unsigned: fine for testing, but SmartScreen warns on every
other PC. Pro test installers built locally with `LM_OPENROUTER_KEY` carry the owner's key, like
the macOS Pro test builds: never upload one. CI never builds them with a key.
