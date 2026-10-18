# Update feed (WP5.7)

At launch LayeredMap reads one small JSON file from the download host and, when it names a newer
version, shows a quiet notice in the map's lower right: "A new version is available" with a
download link. The app downloads and installs nothing itself, and sends nothing but the request
for the file (no identifiers, no usage data; the User-Agent is `LayeredMap/<version>`).

## When the check runs

- Once per launch, on a background thread (`lm-ui/src/update.rs`), never on the interface thread.
- Only when the build has a feed address: `LM_UPDATE_FEED_URL=https://… cargo build --release`
  (`packaging/lm/bundle-macos.sh` passes the environment through). Without it the check is off.
  The address must be `https://`.
- Through the network gate (`lm-net`): with View › Work Offline on, the request is refused before
  it is made. Without a network, or when the host doesn't answer within 10 seconds, the check is
  skipped silently (one line in the log).
- Not in harness runs (`--feel`, `--launch-probe`, `--tour`).

## Format

```json
{
  "format": 1,
  "version": "0.3.0",
  "released": "2026-11-02",
  "notes": { "en": "Saving and exporting.", "zh": "保存与导出。" },
  "download": {
    "free": "https://download.example.cn/layeredmap/LayeredMap-0.3.0-0-macos-arm64.dmg",
    "pro": "https://download.example.cn/layeredmap/LayeredMap-Pro-0.3.0-0-macos-arm64.dmg"
  },
  "download_windows": {
    "free": "https://download.example.cn/layeredmap/LayeredMap-0.3.0-0-windows-x64-setup.exe",
    "pro": "https://download.example.cn/layeredmap/LayeredMap-Pro-0.3.0-0-windows-x64-setup.exe"
  },
  "sha256": { "free": "…", "pro": "…" },
  "page": "https://download.example.cn/layeredmap/",
  "minimum_macos": "13.0"
}
```

| Field | Required | Meaning |
|---|---|---|
| `format` | yes | The feed format. This document describes `1`; a build ignores a feed of any other format, so a future format can't confuse older builds. |
| `version` | yes | The newest release, `MAJOR.MINOR.PATCH` with an optional `-pre` suffix. Compared numerically with the running version; a pre-release sorts before its release. The notice shows only when this is newer. |
| `released` | no | The release date, `YYYY-MM-DD` (informational). |
| `notes` | no | One line on what changed, by language (`en`, `zh`). Shown when the pointer rests on the notice; Chinese when the system's first language is Chinese and `zh` is present, else English. |
| `download.free`, `download.pro` | no | The .dmg for each tier. LayeredMap links its own tier's. |
| `download_windows.free`, `download_windows.pro` | no | The Windows installer for each tier (WP6.2). A Windows build links its own tier's, else `page`; it never links a .dmg. macOS builds ignore the field. |
| `page` | yes, unless both downloads are given | The download page, linked when the tier has no `download` entry. |
| `sha256` | no | The .dmg's SHA-256 by tier, for people who verify downloads (the app doesn't read it). |
| `minimum_macos` | no | The oldest macOS the new version runs on. On an older macOS the notice is not shown. |

Rules the app applies:

- Every link must be `https://` with a host; any other link is ignored, and with no usable link
  the notice is not shown.
- At most 64 KB is read. Unknown fields are ignored, so fields can be added within format 1.
- The notice can be closed; it comes back at the next launch while the version is still newer.

## Making the feed

`packaging/lm/build-dmg.sh` writes `update-feed.json` next to the .dmg when `LM_DOWNLOAD_BASE`
(where the files will be uploaded) is set, adding one tier per run:

```sh
LM_DOWNLOAD_BASE=https://download.example.cn/layeredmap/ LM_RELEASE_NOTES="Saving and exporting." \
  LM_RELEASE_NOTES_ZH="保存与导出。" packaging/lm/build-dmg.sh dist/lm
LM_TIER=pro LM_DOWNLOAD_BASE=https://download.example.cn/layeredmap/ packaging/lm/build-dmg.sh dist/lm
```

Upload both .dmg files first, then `update-feed.json`, so the feed never points at a file that
isn't there yet.

## Open (owner)

The download host, and with it the feed address and `LM_DOWNLOAD_BASE`, is the owner's decision
(spec › WP5.7: a China-reachable host, not GitHub releases; see `basemap-hosting.md` for the
same choice for tiles). Until it is made, builds have no feed address and the check is off.
