# J6 — Match a table to a layer: one-to-one, one-to-many, many-to-one, many-to-many

Inputs for `lm_jobs::join::expect` and `apply` (WP6.4, docs/lm/actions-p2.md › Match a table
to a layer). Twelve square wards near 121° E, 31° N.

- `wards.geojson` (12 areas): `ward_code` W01–W12, each once.
- `wards_split.geojson` (14 areas): the same wards, plus a second record for W03 and for
  W07 (an island part stored as its own record), so two codes repeat.
- `ward_population.csv` (13 rows): one row per ward for W01–W10 and W12 (W11 has none),
  plus W99 (no such ward) and a row with a blank key.
- `ward_population_by_year.csv` (21 rows): W01–W06 for 2021–2023 (W02's rows out of
  order), W98 twice (no such ward, repeated) and W12 for 2023 only.

The four pairings cover pandas `validate`'s four cases: wards × one row each is one-to-one,
wards × by year one-to-many, split wards × one row each many-to-one, split wards × by year
many-to-many.

Expected (from `tools/lm/reference_tests/join_validate.py`):

- `expected_join.csv`, one row per pairing: the cardinality (the tightest `validate` that
  pandas accepts), the rows of a plain left merge (`merge_rows`) and an inner merge (`pairs`),
  layer records with data, table rows with a place, the unmatched keys in table order and the
  unmatched rows (1-based data rows, `|` between them), and the blank keys.
- `expected_join_values.csv`: the population each layer record gets (blank: none). With one
  row per ward it is the merge's value; with several, the row with the latest year.

Keys are compared exactly; blank keys never match (pandas would match a blank with a blank) and
are left out of the cardinality check. Tolerances: everything exact.

Made by `tools/lm/reference_tests/make_p2_fixtures.py` (synthetic, CC0-1.0).
