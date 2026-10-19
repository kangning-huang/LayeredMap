# The app's name

**NimbleMap** (Pro tier: **NimbleMap Pro**; Chinese name **千层图**) is the app's working name since 2026-10-19. It was
called LayeredMap before. The trademark check is still pending, so the name may change again.
This page lists where the name lives, so a second rename (or an undo) takes a few edits.

## Where the display name is set

| Place | What to edit |
|---|---|
| Rust code | `crates/lm-core/src/brand.rs`: `NAME`, `PRO_NAME` (plus `name(pro)` and `with_version`), and `NAME_ZH` (千层图; not shown in the app until the Chinese interface, WP5.5) |
| Rust literals that can't read a `const` | Each has a comment that names `lm_core::brand`: `git grep -n "lm_core::brand"`. They are the `OFFLINE` messages in `lm-ask` and `lm-net`, the `User-Agent` strings (`concat!`) in `lm-net` and `lm-jobs`, the PDF creator and producer in `lm-mapview` (which doesn't depend on lm-core), and two `&'static str` KML exclusion reasons in `lm-jobs` |
| macOS bundle, Info.plist | `brand=` at the top of `packaging/lm/bundle-macos.sh` |
| Disk image | `brand=` at the top of `packaging/lm/build-dmg.sh` |
| Windows installer | `#define Brand` at the top of `packaging/lm/windows/layeredmap.iss`; `$brand` at the top of `packaging/lm/windows/build-installer.ps1` (the installer's file name) |
| Windows version information | `BRAND` at the top of `apps/layeredmap/build.rs` |
| Windows application manifest | `<description>` in `packaging/lm/windows/layeredmap.exe.manifest` |
| Speech helper messages | `appName`, `proName` at the top of `packaging/lm/dictate.swift` |
| Assistant system prompt | `RULES` in `tools/lm/llm_eval/v2/prompt_v2.py`, then `python -I tools/lm/assistant/export_assets.py` (writes `crates/lm-ask/assets/system.txt`) |
| Website and in-app help | Every page in `web/lm/` and `web/lm/zh/`, `web/lm/assets/glossary-data.js` and `web/lm/README.md`, as plain text. English pages say NimbleMap. Chinese pages say 千层图, introduced once per page as "千层图（NimbleMap）", with JSON-LD `name` 千层图 and `alternateName` NimbleMap; they keep the English menu item "Help › NimbleMap Help", as the interface is English. In the glossary, `*_zh` strings say 千层图 and `*_en` strings NimbleMap |
| CI artifact names | `name:` of the `upload-artifact` steps in `.github/workflows/lm-ci.yml` |
| Docs | `README.md`, `docs/lm/spec.md` (title and forward-looking text) and the user-facing pages in `docs/lm/` |

After a rename, the approved UI snapshots that show the name in the toolbar must be re-approved
(`UPDATE_SNAPSHOTS=1 cargo test -p lm-ui screenshots`), and so must the import-notice snapshots
(`LM_UPDATE_SNAPSHOTS=1 cargo test -p lm-jobs snapshot`). Compare each new image with the old one
first: only the name may differ. Screenshots on the website (`web/lm/img/`) are retaken by hand.

## What keeps the old name, and why

These are identifiers, not display text. Changing them would break saved data, installed copies,
keys, permissions or upstream merges.

- **Crates, packages and the binary:** `lm-*`, the directory `apps/layeredmap`, the executable
  `layeredmap` (`layeredmap.exe`, `layeredmap-pro.exe`), the repository name.
- **Bundle identifiers and types:** `app.layeredmap.desktop`, `app.layeredmap.pro` and the
  UTType `app.layeredmap.map`, the MIME type `application/vnd.layeredmap.map+zip`. Changing a
  bundle id makes macOS treat the app as a new one (lost permissions, window state and file
  associations).
- **The `.lmap` extension and its format docs** (`docs/lm/format-v1.md`).
- **Windows identifiers:** the installers' AppIds, the ProgIDs `LayeredMap.*` and
  `LayeredMapPro.*`, the manifest's `assemblyIdentity`. The install folder does follow the name
  (`…\Programs\NimbleMap`); an upgrade over an existing install keeps its folder, as Inno Setup
  reuses the previous folder for the same AppId.
- **User folders** (`lm_core::platform`): `~/Library/Application Support/LayeredMap`,
  `~/Library/Logs/LayeredMap`, `~/Library/Caches/LayeredMap`, `%APPDATA%\LayeredMap`,
  `%LOCALAPPDATA%\LayeredMap\…`, `~/LayeredMap/logs`, and the temporary folders
  `LayeredMap`, `LayeredMap-logs` and `LayeredMap packages`. **Moving these is a separate
  task:** it needs a one-time migration of autosaves, the recovery journal, the question log and
  the caches.
- **Keychain and environment:** the keychain item (service `openrouter`, account `layeredmap`),
  the scrambling pad for the test builds' built-in key (`LayeredMap Pro test build: …` in
  `lm-ask/src/openrouter.rs`, `bundle-macos.sh` and `build-installer.ps1`, which must agree), and
  every `LM_*` environment variable.
- **Other identifiers:** the PDF's fallback font name `LayeredMapFont`, the `layeredmap.app`
  domain in the assistant's `HTTP-Referer`, the CI workflow and job names (they are the status
  checks branch rules name), and a hash test vector in `lm-jobs/src/fingerprint.rs`.
- **Records:** `docs/lm/decisions.md`, the spec's phase records (Phases 0–6), `gates/`,
  `docs/lm/reviews/`, `docs/lm/netcdf-spike.md`, the benchmark sources and their converters
  (`tools/lm/benchmarks/`, `docs/lm/benchmarks/coverage.md`) and the evaluation results in
  `tools/lm/llm_eval/results/`.
- **Fixtures and archives:** `reference/`, `fixtures/`, `validation/`, `spikes/`, `design/`
  (including the logo files) and `web/lm/assets/logo.svg`.
- **Upstream RustGIS files:** the `LayeredMap:` notes in files that come from RustGIS
  (`Cargo.toml`, `.gitignore`, `xtask/src/main.rs`, `xtask/Cargo.toml`, `crates/assistant`,
  `crates/data`, `crates/map`, `crates/render`, `crates/tiles`), kept to make upstream merges
  simple.
