# Task card template

```markdown
# WP2.3 · task 2 of 5 — CSV coordinate detection
Read first: docs/lm/spec.md → Assumption catalog, rows 1–2; crates/lm-profile/src/csv.rs
Do: detect lon/lat column pairs by name, range and parse rate; emit an Assumption
    with confidence and evidence; pick the visible line or the question card per the grid.
Acceptance: snapshots for T1 and T2 match; unit tests for 12 header variants,
    including 经度/纬度 and swapped columns; nothing in the source file changes.
Out of scope: Excel, GeoPackage, the Change popover UI.
Show me: a 30-second recording opening T1 and T2, plus the profile JSON.
Done when: CI is green and the owner approves the snapshots.
```

# Phase kickoff prompt

```markdown
We are starting Phase N of the LayeredMap build plan (docs/lm/spec.md).
The last gate tag is gate-(N-1)-pass. Read the Phase N section, the sections it cites,
and AGENTS.md. Then: (1) list every work package as task cards using the template,
ordered by dependency; (2) list anything in the plan that is ambiguous or contradicts
the code, as questions for me; (3) do not write code until I approve the cards.
```
