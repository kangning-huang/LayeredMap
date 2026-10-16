# Business case (pointer)

The business case for LayeredMap's two tiers lives in the Claude Doc **LayeredMap Business Case**:
https://claude.ai/code/artifact/1dbc13c1-79f8-4fb9-9e72-1b2603bdd3ba (as of 2026-10-16).

In short: **LayeredMap Free** (this repository, Apache-2.0) opens, overlays and maps any common
geospatial file for people with no GIS training. **LayeredMap Pro** (closed, monthly subscription)
adds an assistant: people ask questions about their layers in plain words, the model picks
LayeredMap's vetted analysis actions (it never computes numbers itself), and every answer arrives
as a new layer with its scope, assumptions and recipe. No bring-your-own key for now; the model is
hosted behind a LayeredMap assistant service. Analysis tools have no menus or buttons; the Ask box
is their only door.

The plan's changes are in `docs/lm/spec.md`: "Product tiers", the Phase 4 revision note, and
"Phase 7 — LayeredMap Pro". Decisions are logged in `docs/lm/decisions.md`.
