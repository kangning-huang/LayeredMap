# Contributing to NimbleMap

Thanks for helping. This page covers building and testing NimbleMap on macOS or Windows, and what
a pull request needs. Feedback from *using* the app (an idea, a confusing screen, a file that opens
badly) is just as welcome: open an issue in the public
[NimbleMap](https://github.com/kangning-huang/NimbleMap/issues) repository.

## Where things are

- **NimbleMap's own code** is in `crates/lm-*` and `apps/layeredmap`. The other crates are the
  [RustGIS](https://github.com/opengeos/RustGIS) engine it builds on (`UPSTREAM.md`); change them
  only when NimbleMap needs it, and mark the change `upstream:` in the commit message.
- **The plan** is `docs/lm/spec.md`, and the choices made along the way are in
  `docs/lm/decisions.md`. A change that goes against a logged decision should say so and why.
- The crates, the binary (`layeredmap`) and the bundle identifiers keep the project's former name,
  LayeredMap (`docs/lm/naming.md`). Use `lm_core::brand` for the app's name in anything a person
  sees; never type "NimbleMap" into interface text.
- `AGENTS.md` holds the engine's coding rules (no `unwrap()` in non-test code, no `unsafe`, keep
  the crates below `apps/` building for WebAssembly). They apply to NimbleMap's crates too.

## Set up

You need Rust 1.90 or newer (`rustup update`) and a C compiler.

**macOS:** `xcode-select --install`, then

```bash
git clone https://github.com/kangning-huang/LayeredMap.git
cd LayeredMap
cargo run -p layeredmap --release
```

**Windows 10 or 11 (x64):** install the Visual Studio Build Tools ("Desktop development with C++")
and Rust's MSVC toolchain, and set `core.autocrlf false` and `core.longpaths true` in Git
**before cloning**. The steps, with `winget` commands, are in
[`docs/lm/windows.md`](docs/lm/windows.md) › Building. Then the same `cargo run`.

The first build takes a while (10–20 minutes on a laptop); later ones are incremental.

## Test

```bash
cargo xtask ci
```

That is the whole check, the same one CI runs on macOS and Windows: formatting
(`cargo fmt --all --check`), lints (`cargo clippy -- -D warnings`), every test, and the layering
rules (the interface may not do spatial math itself; NimbleMap's crates may only depend downward).
Run it before every commit. If it's green on your machine and red in CI, say so in the pull
request.

**Interface changes and snapshots.** The interface is tested against approved screenshots in
`crates/lm-ui/tests/snapshots`. They run on macOS only, because the images depend on the Mac's
GPU and text rendering. If your change alters what the interface looks like:

- On a Mac: `UPDATE_SNAPSHOTS=1 cargo test -p lm-ui screenshots`, look at every changed image,
  and commit the ones you meant to change.
- On Windows: you can't re-approve them. Say in the pull request that the interface changed, and
  attach a screenshot of the change; a maintainer re-approves the snapshots on a Mac.

**A bug fix comes with a test** that fails before the fix and passes after it.

## Pull requests

1. Fork the repository and create a branch from `main` (`git switch -c fix/csv-semicolons`).
2. Keep a pull request to one change. Small ones are reviewed quickly.
3. Run `cargo xtask ci`.
4. Open the pull request against `main` and fill in the template: what changed in plain words, a
   screenshot for anything visible, the tests you added, and what it could break. Say which
   system(s) you tested on.
5. CI must pass on macOS and Windows before a merge.

Write commit messages and interface text in plain words. The app speaks to people who have never
used GIS: "Located using lon and lat as GPS coordinates", not "CRS inferred: EPSG:4326".

## What not to commit

- **Keys and passwords.** NimbleMap is one app: Pro's features unlock when someone signs in, and
  the Ask box goes through NimbleMap's assistant service, which holds the model key. No build
  carries an OpenRouter key. For prompt work a developer can use their own key
  (`LM_ASK_ROUTE=direct`, the key from your Mac's keychain or, on Windows, the
  `OPENROUTER_API_KEY` environment variable); it must never appear in the repository, a test or a
  log. `LM_DEV_PRO=1` unlocks Pro's local features without an account, for development only.
- **Data you can't share.** Test fixtures must be generated (`cargo xtask fixtures`) or carry a
  licence that allows redistribution; record their source in `ATTRIBUTION.md`.
- **Build output and system files:** `target/`, `dist/`, `.DS_Store`, and the `._*` files macOS
  writes on external drives.

## Licence

This repository is licensed under Apache-2.0 (`LICENSE`). By opening a pull request you agree that
your contribution is licensed the same way.
