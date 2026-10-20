# Unify Free and Pro: status (work stopped, uncommitted)

The work stopped on 2026-10-19 because the usage limit was reached. Nothing is committed on
`unify`, because CI hasn't passed for these changes yet. All the changes are uncommitted in this
worktree.

## Done (code compiles, clippy is clean)
- **Entitlement model** (`crates/lm-account/src/entitlement.rs`):
  - `Entitlement` holds `pro`, `plan`, `expires_at` and `checked_at`.
  - `Access` has six states: Unlocked, Developer, SignedOut, Checking, NotPro and Unavailable.
  - `entitlement::access()` is the one answer. `developer_override()` reads `LM_DEV_PRO=1`.
- **The entitlement is kept with the session.** It is stored in the keychain record, as optional
  keys in record v1. It also survives a token refresh.
- **New `Account` methods:** `refresh_entitlement()` (GET `…/account`; 404 counts as not Pro),
  `note_ask_answer()`, `access()`, `entitlement()`, `entitlement_due()` and `lm_account::now()`.
- **Tests:** the lm-account unit tests and `tests/mock_auth.rs` pass. A mock GET `/account` and a
  new entitlement test were added.
- **lm-ask:**
  - The bundled key path is removed: `assistant.dat`, `PAD`, `internal_build` and their test.
  - `proxy::is_not_pro()` is added.
- **lm-core brand:** `name(pro)` is removed. The app title is always `NAME`. `PRO_NAME` stays as
  the name of what signing in unlocks.
- **lm-ui:**
  - `Shell.pro` becomes `Shell.dev_pro`. New `pro_access()`, `pro()`, `analysis_offered()`,
    `analysis_unlocked()` and `lm_ui::dev_pro_override()`.
  - The PRO badge shows only when Pro is unlocked.
  - Ask box: when locked, a prompt replaces the field and the microphone. The prompt reads "Sign
    in to use …", "… isn't on for this account." with Settings…, or "Checking your account…".
    `submit` and `dictate` are gated. A proxy answer updates the entitlement.
  - Card actions: when locked, the analysis actions show with a lock (`illustrations::button_locked`).
    Their names end in "(locked)" and they carry a lock line with a Sign In… link. Clicking one
    opens Settings › Account (`account_ui::locked`). `analysis::open` is gated.
  - Settings › Account: the signed-out pane says what signing in unlocks. The signed-in pane shows
    "on", "not on" or "checking".
  - The entitlement check runs from `account_ui::poll`: once at launch, after signing in, every
    6 h, and a retry after 60 s.
  - The update feed has one app, read from the "free" key. The `pro` parameter is removed.
- **App (`main.rs`):** `is_pro` and the tier file are removed. The app id is always
  `app.layeredmap.desktop`. The account starts on every non-ephemeral launch.
- **Catalogs:** 5 new strings, added to all ten catalogs.
- **Tests and snapshots:** updated to the new model. New snapshots: `locked_layer_panel`,
  `ask_not_pro_prompt`, and an a11y scene.

## Left to do
1. Run `cargo test -p lm-ui` again after the last edit, which kept the sign-in prompt's layout.
   Then look at every `*.new.png` and `*.diff.png` in `crates/lm-ui/tests/snapshots/`:
   - **Expected changes:**
     - `ask_sign_in_prompt`: no PRO badge, and Share is disabled because export follows `pro()`.
     - `account_*` (signed-out ones): the new "Signing in unlocks…" line, and no badge.
     - `locked_layer_panel` and `ask_not_pro_prompt`: new.
   - **Investigate:** `phase3_join_question`, `phase3_join_confirm` and
     `a11y::keyboard_only_open_an_example_select_and_hear_the_summary` also failed. These are Free
     surfaces with no account config, so check whether they also fail on d3a4ec3 (flaky or
     environment-related) or whether something regressed.
   - Approve with `UPDATE_SNAPSHOTS=1` only after looking at each image.
2. Packaging still has to drop the tier:
   - `bundle-macos.sh`: always build `lm-dictate` and include the reference data and `account.json`
     (warn when it's missing). Use the Free bundle id. Rename `pro.entitlements` to
     `app.entitlements` and sign both the helper and the app with it. Remove the key python.
   - `build-dmg.sh`: drop `LM_TIER` and write the feed under both `free` and `pro`, pointing at the
     one dmg.
   - Windows `build-installer.ps1` and `layeredmap.iss`: keep one AppId (Free), one exe and the
     `LayeredMap` ProgIDs. Remove `LM_OPENROUTER_KEY`.
   - `tools/lm/puzzles/pro_e2e.py` and the other tools: change `LM_TIER=pro` to `LM_DEV_PRO=1`.
3. Help pages and the site (`web/lm`, all languages): change "separate Pro download" to "sign in
   to unlock Pro (free while it's new)". The `tools/lm/site` tests must pass.
4. Docs to update:
   - `accounts-design.md`: add an Entitlement section. Note that local Pro features are locked in
     the interface only, while the Ask box is enforced on the server. Update the route table and
     drop the "When the test-build key goes" section.
   - Also `accounts.md`, `app-store.md` (the in-app purchase feeds the same `Entitlement`), the
     `spec.md` Product tiers note, a `decisions.md` entry, `signing.md`, `windows.md`,
     `update-feed.md`, `naming.md`, `CONTRIBUTING.md` and `README.md`.
5. **Migration note:**
   - The old Pro app kept its window and settings in `~/Library/Application Support/app.layeredmap.pro`
     (eframe storage). These are not migrated; people delete them along with the app.
   - The keychain item `app.layeredmap.pro.account` is shared and kept, so Pro testers stay signed
     in.
   - `~/Library/Application Support/LayeredMap` (autosave, question log, `account.json`) and the
     logs were already shared.
6. Before committing:
   - Delete the `._*` files.
   - `cargo xtask ci`, the pytest run (`tools/lm/llm_eval` and `tools/lm/site`) and
     `services/ask-proxy` `npm test` must all pass.
   - Run the key grep.
