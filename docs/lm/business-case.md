# Business case (pointer)

The business case for NimbleMap's two tiers lives in the Claude Doc **LayeredMap Business Case**:
https://claude.ai/code/artifact/1dbc13c1-79f8-4fb9-9e72-1b2603bdd3ba (as of 2026-10-16).

In short: **NimbleMap Free** (this repository, Apache-2.0) opens, overlays and maps any common
geospatial file for people with no GIS training. **NimbleMap Pro** (closed, monthly subscription)
adds an assistant: people ask questions about their layers in plain words, the model picks
NimbleMap's vetted analysis actions (it never computes numbers itself), and every answer arrives
as a new layer with its scope, assumptions and recipe. No bring-your-own key for now; the model is
hosted behind a NimbleMap assistant service. Update 2026-10-21 (owner's decision, not yet in the
Claude Doc): the basic analysis actions are Free, as buttons on cards with a composer, preview and
Keep, since they are local, certified algorithms; Pro is the Ask box (the LLM), whose per-question
model cost is what Pro covers.

The plan's changes are in `docs/lm/spec.md`: "Product tiers", the Phase 4 revision note, and
"Phase 7 — NimbleMap Pro". Decisions are logged in `docs/lm/decisions.md`.
