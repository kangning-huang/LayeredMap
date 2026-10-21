# A1 — An Ask answer: storm tracks within 100 km of a city

A tiny map for the Ask box's preview and Keep (owner's bug report, 2026-10-20: "How many
Category 5 storms have passed within 100 km of Tokyo?" showed every Category 5 track instead of
the ones that answer). Hand-made, synthetic (CC0-1.0).

- `storms.geojson` (`name`, `strength`, as IBTrACS names it): five straight tracks. Abby
  (Category 5) runs through Tokyo; Near (Category 5) passes about 50 km east of it; Weak
  crosses Tokyo but is Category 3; Far (Category 5, the open Pacific) and Wide (Category 5,
  about 210 km east) stay outside 100 km.
- `cities.geojson` (`name`): Tokyo (139.69° E, 35.69° N).

Expected, for the plan "filter storms to strength Category 5; buffer Tokyo by 100 km; count the storms
in that area": 4 of 5 storms after the filter; 2 counted (Abby and Near, rows 0 and 3), and
those 2 are what the preview highlights and Keep selects. Distances are geodesic: Near's
nearest point is about 0.55° of longitude (about 50 km) from Tokyo, Wide's about 2.4° (about
215 km).

Used by `crates/lm-jobs/src/plan_exec/tests.rs`, `crates/lm-ui/src/workspace/analysis.rs`
(Ask preview tests) and the `pro_ask_answer_preview` snapshot.
