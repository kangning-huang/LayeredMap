# Upstream

NimbleMap is a fork of [opengeos/RustGIS](https://github.com/opengeos/RustGIS) (Apache-2.0).
Product code lives only in new crates (`crates/lm-*`, `apps/layeredmap`); changes inside RustGIS
crates are separate commits prefixed `upstream:`. Upstream is merged only at the start of a phase,
after which the full CI suite and the feel harness run.

| Date | Event | Upstream commit | Workspace version | Notes |
|---|---|---|---|---|
| 2026-10-15 | Baseline (Phase 0 start) | `75129448c7cd798d836865107a2213e72c8f9367` (2026-10-15 01:48 −04:00, "feat(plugins): process plugins…", #245) | 0.2.0 | Imported as `main`; remote `upstream` |

## Local changes to RustGIS files

| File | Change | Why |
|---|---|---|
| `Cargo.toml` | `spikes/*` added to workspace members | Phase 0 spikes build in the workspace (not default members) |
| `xtask/src/main.rs`, `xtask/Cargo.toml` | `mod lm; mod fixtures;`, the `fixtures` task, `lm::layers` in `layers` and `ci` | NimbleMap's layering rules and fixture factory; logic lives in new files `xtask/src/lm.rs`, `xtask/src/fixtures.rs` |
| `AGENTS.md` | A NimbleMap section appended | Product rules for coding agents |
| `.gitignore` | `/fixtures/generated` | Generated fixtures are large |
| `crates/assistant/src/tests.rs` | `docs_pages_and_addresses` skips `docs/lm` | The fork's build docs live in `docs/lm` (plan paths) and are not RustGIS help pages; not for upstream |
| `.github/pull_request_template.md` | Replaced with NimbleMap's template | Plan › Repository conventions |

## Upstream conversation (WP0.1)

Not yet opened. Draft for the owner to post as a GitHub Discussion on opengeos/RustGIS:
`docs/lm/upstream-discussion-draft.md`.
