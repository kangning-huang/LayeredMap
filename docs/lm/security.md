# Security and licences (WP5.7, WP6.6, WP6.8)

What runs where, and how to run it by hand.

| Check | Command | Where it runs | Needs network |
|---|---|---|---|
| Network gate lint: no `ureq` without `lm_net` | part of `cargo xtask layers` / `ci` | every CI run | no |
| Network-blocking harness (Gate 5) and strict offline mode (WP6.6) | part of `cargo test` (`lm-ui` › `offline_harness`) | every CI run | no |
| Known vulnerabilities (RustSec) | `cargo audit` | CI `security` job; `cargo xtask audit` | yes (advisory database) |
| Licences, bans, sources, advisories | `cargo deny check` (`deny.toml`) | CI `security` job; `cargo xtask audit` | yes |
| Crate licence texts (ATTRIBUTION) | `cargo xtask licenses` → `ATTRIBUTION-crates.md` | by hand, before a release | no |
| Software bill of materials | `cargo xtask sbom [--out FILE] [--target TRIPLE]` | CI `security` job (artifact `LayeredMap-sbom`) | no |
| Fuzzing, smoke | `cargo xtask fuzz --secs 60` | by hand | no |
| Fuzzing, 24 h per adapter (Gate 6) | `.github/workflows/lm-fuzz.yml` (manual) | Linux runners | no |

Tools: `cargo install --locked cargo-audit cargo-deny`, `cargo install --locked cargo-about
--features cli`, and for fuzzing `rustup toolchain install nightly && cargo install cargo-fuzz`.

## The network gate and strict offline mode

Every network request in LayeredMap asks `lm_net::allowed(purpose)` right before it is sent:

| Request | Where | Purpose |
|---|---|---|
| Basemap and imagery tiles (every remote read of the tile fetcher) | `rustgis-tiles` `cache::native::get`, through the gate hook `lm-mapview` installs | Basemap |
| Newest daily basemap build (`builds.json`, then HEAD requests) | `lm-mapview` `archive` | Basemap |
| Pro's assistant (OpenRouter) | `lm-ask` `openrouter` | Assistant |
| Update feed | `lm-ui` `update`, through `lm_net::get_text` | UpdateCheck |
| Data library (WP5.4, not on this branch yet) | must use `lm_net::get_text` or ask `allowed(Purpose::Library)` | Library |

Not network: place search reads the bundled GeoNames index. Dictation uses Apple's recognizer
on the Mac; offline it refuses to fall back to Apple's server. RustGIS's own network code (the
plugin registry, the AI Assistant, the GeoLibre installer, S3/STAC/portal commands) is not
reachable from LayeredMap; the engine's remote reads go through the same tile fetcher, so they
would meet the gate too.

View › Work Offline turns strict offline mode on: every purpose is refused, the map draws the
offline outline, the Ask box says why it can't answer. The harness
(`crates/lm-ui/src/offline_harness.rs`) runs open, place search, select, an action and Keep,
export, the diagnostics review and (Pro) Ask in a child process whose HTTP clients all point at
a local tripwire, and checks that the gate allowed nothing and the tripwire saw nothing. A
second run, online with the offline basemap, checks that the core tasks never even ask; a third
proves the tripwire catches a request that does go out.

## Advisories

`cargo audit` and `cargo deny check` are clean apart from the exceptions in `.cargo/audit.toml` and
`deny.toml`, each with its reason (decisions, 2026-10-18). Unmaintained crates that arrive
transitively (ttf-parser through egui, zbus on Linux) are reported as warnings.

## Fuzzing

Every import adapter has a libFuzzer target in `fuzz/` (`xtask` fails when an adapter has none):

| Target | Adapter | Seeds |
|---|---|---|
| `csv` | CSV and TSV | fixtures' `.csv`, X5 |
| `geojson` | GeoJSON | fixtures' `.geojson`, X3 |
| `shapefile` | Shapefile, zipped Shapefile (first byte picks the companions) | fixtures' `.shp`, `.zip`, X1, X2 |
| `dbf` | Shapefile attributes and encoding detection | fixtures' `.dbf` |
| `geotiff_header` | GeoTIFF | fixtures' `.tif`, X4 |
| `gpkg` | GeoPackage | fixtures' `.gpkg` |
| `xlsx` | Excel | fixtures' `.xlsx` |

Smoke test (all targets at once, each on its own core):

```sh
cargo xtask fixtures --only X1,X2,X3,X4,X5   # seeds for the hostile fixtures
cargo xtask fuzz --secs 60
```

**24 hours per adapter** (Gate 6, WP6.8) in shards, because a hosted runner stops a job at 6 hours:
run the *LayeredMap fuzz* workflow (Actions › LayeredMap fuzz › Run workflow). It starts 5 shards
of 4 h 50 min for each of the 7 targets (35 jobs, ≈ 24 h 10 min per target) and uploads each
shard's log and any crash. Locally, one 6-hour shard of one target:

```sh
cargo xtask fuzz --secs 21600 --target gpkg
```

Shards of the same target can run on different machines at once; libFuzzer picks a random seed
per run, and `fuzz/corpus/<target>` keeps what each run found (copy corpora between machines to
share coverage). A crash leaves its input in `fuzz/artifacts/<target>/`; reproduce it with
`fuzz/target/<host>/release/<target> <file>`, fix it, and add the file to `fuzz/regressions/`.

### Smoke results

2026-10-18, reference Mac (M-series, all 7 targets at once while other builds ran, so the counts are
low), `cargo xtask fuzz --secs 60`:

| Target | Seconds | Executions | Crashes |
|---|---|---|---|
| csv | 60 | 3,119 | 0 |
| geojson | 60 | 11,279 | 0 |
| shapefile | 60 | 8,595 | 0 |
| dbf | 60 | 11,780 | 0 |
| geotiff_header | 60 | 21,799 | 0 |
| gpkg | 60 | 24,298 | 0 |
| xlsx | 60 | 48,790 | 0 |

A smoke test only: the 24-hour runs (Gate 6) have not been done.
