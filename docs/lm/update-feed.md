# Updates: the feed, Update Now and the signing key (WP5.7)

At launch, and when the user chooses **NimbleMap › Check for Updates…** (**Help › Check for
Updates…** on Windows), NimbleMap reads one small JSON file and, when it names a newer version,
shows a quiet notice in the map's lower right: "A new version is available", a download link and,
on macOS, **Update Now**. Update Now downloads the new disk image, verifies it, installs it and
reopens the app. The user never has to go back to the website.

**"The map" updates with the app.** The example maps (File › Open Example) and the built-in world
layers ship inside the app bundle, so every app update brings their new versions. The online
basemap is served live and is always current. There is no separate data updater.

Code: `crates/lm-ui/src/update.rs` (feed, state, notice), `update/fetch.rs` (the two requests),
`update/verify.rs` (signature, checksum, the public key), `update/install.rs` (macOS install).
Tools: `cargo xtask update-keygen`, `cargo xtask update-sign` (`xtask/src/update.rs`).

## Where the feed lives

| | Address | Compiled in from |
|---|---|---|
| Feed | `https://nimblemap.com/update/feed.json` | `LM_UPDATE_FEED_URL` |
| Fallback | `https://api.gitcode.com/api/v5/repos/KN-Huang/NimbleMap/raw/docs/update/feed.json?ref=main` | `LM_UPDATE_FEED_FALLBACK_URL` |

- The file is `web/lm/update/feed.json` in this repository. `tools/lm/site/stamp.py` copies it with
  the site into the public NimbleMap repo's `docs/`, so GitHub Pages serves it at the first address,
  and the GitCode mirror of that repo serves it at the second. One file, two hosts.
- The fallback is read only when the first address fails (no answer in 10 s, an error, or a block).
  Mainland China may not reach nimblemap.com (GitHub Pages behind Cloudflare) or GitHub reliably.
  GitCode's `raw.gitcode.com` and `gitcode.com/…/raw/…` pages don't serve files to programs (403 or
  an HTML page; checked 2026-10-21); the v5 API's `raw` endpoint does, without a token.
- `packaging/lm/bundle-macos.sh` compiles both in **by default**. A plain `cargo build` has neither,
  so development builds and tests never check. Set either variable to `none` to leave it out.
- `packaging/lm/windows/build-installer.ps1` still takes `LM_UPDATE_FEED_URL` from the environment
  (CI passes the repo variable). Windows only ever links the download.

## When the check runs

- Once per launch, on a background thread, never on the interface thread; and on demand from the
  menu (which then also says "You're up to date: NimbleMap 0.7.2 is the newest version." or
  "Couldn't check for updates…").
- Through the network gate (`lm-net`): `Purpose::UpdateCheck` for the feed and
  `Purpose::UpdateDownload` for the disk image. With View › Work Offline on, both are refused before
  anything is sent. Only `https://` addresses are fetched, and redirects must stay on `https://`.
- Not in harness runs (`--feel`, `--launch-probe`, `--tour`); a tour can run it with the
  `check-updates` and `update-now` steps (the dry run below).

## Format

Format 1, extended in 0.7.2 with fields older builds ignore (unknown fields are ignored, so a 0.7.2
feed is still readable by 0.7.0 and 0.7.1 if they had a feed address, and a feed without the new
fields is still readable by 0.7.2: it gets the link, not Update Now).

```json
{
  "format": 1,
  "version": "0.7.2",
  "released": "2026-10-22",
  "notes": { "en": "In-app updates.", "zh": "应用内更新。" },
  "download": {
    "free": "https://github.com/kangning-huang/NimbleMap/releases/download/v0.7.2/NimbleMap-macos-arm64.dmg",
    "pro": "https://github.com/kangning-huang/NimbleMap/releases/download/v0.7.2/NimbleMap-macos-arm64.dmg",
    "gitcode": "https://gitcode.com/KN-Huang/NimbleMap/releases/download/v0.7.2/NimbleMap-macos-arm64.dmg"
  },
  "sha256": { "free": "<64 hex digits>", "pro": "<64 hex digits>" },
  "signature": "<base64 Ed25519 signature>",
  "page": "https://nimblemap.com/download.html",
  "minimum_macos": "13.0"
}
```

| Field | Required | Meaning |
|---|---|---|
| `format` | yes | `1`. A build ignores a feed of any other format. |
| `version` | yes | The newest release, `MAJOR.MINOR.PATCH` with an optional `-pre` suffix, compared numerically (a pre-release sorts before its release). Only a newer version is offered. |
| `released` | no | `YYYY-MM-DD`, informational. |
| `notes` | no | One line by language (`en`, `zh`), shown when the pointer rests on the notice. Chinese when the **interface language** is Chinese and `zh` exists. |
| `download.free` | for Update Now | The .dmg on **GitHub**. Every build reads it; `pro` is the same file for Pro apps from 0.7.0 and earlier. |
| `download.gitcode` | no (0.7.2+) | The same .dmg on the **GitCode** mirror. |
| `download_windows.free` | no | The Windows installer; Windows links it (else `page`), never a .dmg. |
| `sha256.free` | for Update Now | The .dmg's SHA-256 (hex; read case-insensitively). |
| `signature` | for Update Now (0.7.2+) | Ed25519 over the version and `sha256.free` (below), base64. |
| `page` | unless a download is given | The download page. |
| `minimum_macos` | no | On an older macOS the notice is not shown. |

The link in the notice is GitCode's when the interface is Chinese, else GitHub's, else `page`.

### Which download Update Now uses

GitCode first when the interface language is Chinese **or** the feed itself only came through the
GitCode fallback (so GitHub-hosted addresses are probably unreachable too); GitHub first otherwise.
If the first download fails (not when the user cancels, and not when offline), the other is tried.

## Update Now (macOS)

1. **Refuse before downloading.** The offered version must be newer than the running one (no
   downgrades, no reinstalling the same version), and `signature` must verify against the public key
   compiled into the app (`PUBLIC_KEY` in `update/verify.rs`) over these exact bytes:

   ```text
   NimbleMap update\nformat 1\nversion <version>\nsha256 <sha256, lowercase hex>\n
   ```

   A compromised host, mirror or DNS can serve another file or feed but can't produce that
   signature (the Sparkle model). URLs, notes and dates aren't signed: any address that serves bytes
   with the signed checksum is fine.
2. **Download** to `~/Library/Caches/LayeredMap/Updates/NimbleMap-<version>.dmg` (via a `.part`
   file), with a determinate progress bar and **Cancel**. At most 2 GB.
3. **Verify** the file's SHA-256 against the signed one; a mismatch deletes it ("The downloaded file
   is damaged…, so nothing was installed.").
4. **Where does the app run?** If it isn't in an app bundle, runs from the disk image itself (a
   read-only `/Volumes/…`), from App Translocation (`…/AppTranslocation/…`), or its folder isn't
   writable (a standard user and /Applications), it doesn't try: it opens the downloaded disk image
   in Finder with a short note ("…drag NimbleMap to Applications…").
5. **Stage.** `hdiutil attach -nobrowse -readonly -noautoopen -noverify` on a private mount point;
   `ditto` the app into a hidden sibling of the running bundle (`.NimbleMap-update-<pid>`, no `.app`,
   so Launch Services ignores it); check the copy: Info.plist `CFBundleShortVersionString` equals the
   feed's version, `CFBundleIdentifier` equals the running app's, `codesign --verify --strict` passes,
   and the signing **Team ID is `75MXK7UJ6W`** (the owner's Developer ID; ad-hoc signatures are
   accepted only in tests and `update-test-hook` builds). `hdiutil detach`. Nothing has changed yet;
   any failure removes the copy ("The new version couldn't be installed, and nothing was changed.").
6. **Quit.** The notice says "NimbleMap 0.7.2 is ready" and the window is asked to close, exactly as
   ⌘Q does: with unsaved changes, "Do you want to save the changes…?" comes first (Cancel keeps the
   app open with **Restart Now** in the notice). The app saves its preferences and window and marks
   the recovery journal clean as on any quit.
7. **Swap** (in `on_exit`, after the state is saved): rename the running bundle to
   `.NimbleMap-old-<pid>`, rename the staged copy to `NimbleMap.app` (if that fails, the old one is
   renamed back and the disk image opens in Finder), note the old copy's path in
   `~/Library/Application Support/LayeredMap/update-cleanup.txt`, and `open -n` the new app.
8. **Clean up.** At launch the new version removes the old copy named in that file (only a path
   named `.<name>-old-<digits>`) and the downloaded disk image.

A staged update that wasn't restarted (the user cancelled "Save changes?" and later quits) is still
installed at that quit, without reopening.

### Gatekeeper and quarantine

NimbleMap doesn't set `LSFileQuarantineEnabled`, so the files it downloads itself carry no
`com.apple.quarantine` attribute, and neither does the app `ditto` copies out of the disk image.
Gatekeeper only assesses quarantined apps at first launch, so the updated app opens without the
"Open Anyway" step, even when it isn't notarized. The integration test and the dry run check the
attribute is absent. From 0.7.2 releases are also Developer ID signed and notarized, and the Team ID
check above relies on that.

### Windows

Unchanged: the notice links the installer (`download_windows.free`) or the page. No Update Now.

## Privacy

The requests carry no identifiers: no install id, account, language, OS details or cookies. The
only header beyond the HTTP basics is `User-Agent: NimbleMap/<version>`. The hosts (GitHub Pages
and Cloudflare for nimblemap.com; GitCode; GitHub's and GitCode's release CDNs) see the requesting
IP address, as for any download. Nothing is sent about what the user does with the notice.

## The signing key

- **Made once** with `cargo xtask update-keygen` (done 2026-10-21). It generates an Ed25519 key
  pair with `ring`, stores the **private key only in the macOS login keychain** as a generic
  password (service `nimblemap-update`, account `ed25519`, value: base64 PKCS#8), and writes the
  public key into `PUBLIC_KEY` in `crates/lm-ui/src/update/verify.rs`. The key is passed to
  `security -i` on standard input, never as an argument; neither tool prints it or writes it to a
  file. Running it again refuses while a key exists.
- **Public key:** `bxaUHigI18sLRPoijV2wQFTWDofahlGWtGPSyNJaZag=`.
- **Back it up now.** If the private key is lost, every installed copy (0.7.2 and later) rejects
  every future update, and users have to download a new version by hand once. To back it up, copy
  the keychain item's value into a password manager or an encrypted vault, without it touching a
  file or the terminal history:

  ```sh
  security find-generic-password -s nimblemap-update -a ed25519 -w | pbcopy
  # paste into the password manager entry "NimbleMap update signing key", then:
  pbcopy < /dev/null
  ```

  Or Keychain Access › login › "NimbleMap-update-signing-key" › Show Password. To restore it on
  another Mac: `security add-generic-password -s nimblemap-update -a ed25519 -w` (prompts for the
  value).
- **If it leaks**, someone who also controls a download host could push an app to 0.7.2+ users
  (the Team ID check still stops anything not signed with the owner's Developer ID). Make a new key
  pair, ship it in a release, and tell users.

## Release checklist (updates)

After building, signing and notarizing the release (`packaging/lm/build-dmg.sh` with
`LM_SIGN_IDENTITY="Developer ID Application: Kangning Huang (75MXK7UJ6W)"` and
`LM_NOTARY_PROFILE=nimblemap-notary`; `bundle-macos.sh` unlocks
`~/Library/Keychains/nimblemap-signing.keychain-db` itself with the password the login keychain
keeps under `nimblemap-signing-keychain`/`developer-id`, never printed or written down), and after
uploading `NimbleMap-macos-arm64.dmg` and its `.sha256` to the GitHub **and** GitCode releases:

1. `cargo xtask update-sign dist/lm/NimbleMap-macos-arm64.dmg <version> --notes "…" --notes-zh "…"`
   — checks the `.sha256` beside the disk image and the app version inside it, reads the key from
   the keychain, checks it matches the app's public key, and writes `web/lm/update/feed.json`.
2. Commit the feed, stamp the site (`python3 -I tools/lm/site/stamp.py /tmp/site-out`), copy it into
   the NimbleMap repo's `docs/`, and push to **GitHub and GitCode** (`git push gitcode main`). The
   feed must go up only after both disk images are downloadable.
3. Check: `curl -s https://nimblemap.com/update/feed.json` and
   `curl -s 'https://api.gitcode.com/api/v5/repos/KN-Huang/NimbleMap/raw/docs/update/feed.json?ref=main'`
   both show the new version; then Check for Updates… in the previous version offers it.

`packaging/lm/build-dmg.sh` can still write an unsigned `update-feed.json` from `LM_DOWNLOAD_BASE`
(a single host); without a signature that feed gets the link only. Use `update-sign`.

## Builds before 0.7.2

0.7.0 and 0.7.1 were built without a feed address, so they never check: their users need **one
manual download** of 0.7.2 (the website, or the changelog's link). From 0.7.2 on, Update Now does
the rest.

## Dry run (local, not in CI)

Builds with the `update-test-hook` cargo feature also read `file://` feeds and disk images (slowed to
show the progress bar) and pass `LM_UPDATE_TEST_RELAUNCH_ARGS` (one argument per line) to the
reopened app. `bundle-macos.sh` refuses the feature unless the version contains `-test`.

```sh
# 0.7.2-test as the installed app, 0.7.3-test as the update (versions set in Cargo.toml for the run)
LM_CARGO_FEATURES=update-test-hook LM_UPDATE_FEED_URL=file:///tmp/nm-dryrun/feed.json \
  LM_UPDATE_FEED_FALLBACK_URL=none packaging/lm/bundle-macos.sh /tmp/nm-dryrun/Applications
LM_CARGO_FEATURES=update-test-hook LM_UPDATE_FEED_URL=file:///tmp/nm-dryrun/feed.json \
  LM_UPDATE_FEED_FALLBACK_URL=none packaging/lm/build-dmg.sh /tmp/nm-dryrun/v073
cargo xtask update-sign /tmp/nm-dryrun/gitcode/NimbleMap-macos-arm64.dmg 0.7.3-test --out /tmp/nm-dryrun/feed.json \
  --base-github file:///tmp/nm-dryrun/missing --base-gitcode file:///tmp/nm-dryrun/gitcode
LM_UPDATE_TEST_RELAUNCH_ARGS=$'--tour\ncheck-updates;15,30,2.2' \
  /tmp/nm-dryrun/Applications/NimbleMap.app/Contents/MacOS/layeredmap --tour 'check-updates;update-now'
```

The GitHub address points at a missing file, so the run also shows the fallback to GitCode.
