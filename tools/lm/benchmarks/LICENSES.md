# Licences and attribution for the benchmark cases

`sources/*.jsonl` and `suite.jsonl` hold cases converted from public benchmarks. This file
records, for each source, its licence, what we took, and what we changed. The raw downloads are
not in git. They are under `/Volumes/LaCie/data/layeredmap-benchmarks/<name>/`, each with a
README giving the source, licence, commit, date and size.

How much of each source's text we use depends on its licence. Each record's `text_policy`
field says which applies:

- **verbatim:** the original question is unchanged. Only permissive licences (MIT, Apache-2.0).
- **adapted:** the original, rewritten for our map contexts (layer names, units, Chinese
  translation). Only permissive licences.
- **own_words:** no licence, an unclear licence, a non-commercial or GPL licence, or a paper
  without released data. Summaries and requests are our own, written to cover the same
  operations and difficulty. Nothing from the original task text is copied.

No source we used is share-alike (CC BY-SA). If one is added, `build_suite.py` writes its cases
to `suite_sharealike.jsonl`, apart from the main suite. That file would carry the BY-SA licence
and attribution, and any adaptation of it must be shared under the same licence.

## Permissive sources (text used verbatim or adapted)

### GeoBenchX: MIT
- Source: https://github.com/Solirinai/GeoBenchX, commit 0edf6109ea5dca3e70fdfe081d39c9ccd0f832fb (2026-08-29),
  `benchmark_set/tasks_and_reference_solutions.json`.
- Paper: Krechetova and Kochedykov, *GeoBenchX: Benchmarking LLMs for Multistep Geospatial Tasks*, arXiv 2503.18129.
- Changes: task texts are used unchanged, or rewritten to name our layers and fields; Chinese
  translations were added. Reference solutions were replaced by plans over LayeredMap actions.
- Copyright (c) 2025 Varvara Krechetova. MIT licence (full text below).

### UnivEARTH: MIT
- Source: https://huggingface.co/datasets/AaronKao/UnivEARTH, revision 1bbeab5f7c90903459069b86d32bf605fdbd8693
  (2025-04-23), `data.json`, licence on the dataset card: `mit`. The extended set
  https://github.com/IandRover/univearth_arr (commit f8b90781c2b18f17d47933da0a741fc35b8a891a) is
  also MIT; it is not used.
- Paper: Kao et al., *Towards LLM Agents for Earth Observation*, arXiv 2504.12110.
- Changes: questions are used verbatim as refusal cases, and Chinese translations were added.
- Copyright (c) 2025 Chia-Hsiang Kao. MIT licence (full text below).

### ACEBench: MIT
- Source: https://github.com/ACEBench/ACEBench, commit 56dd66cf6439b0d9655ee1b353e4cd745c6f664e
  (2025-10-29), `data_all/data_{en,zh}/`.
- Changes: only geographic items and refusal-pattern items (special_irrelevant,
  special_incomplete) are used. The geographic ones were rewritten for our map contexts, while
  the refusal items are verbatim.
- `LICENSE.md` reads "Copyright (c) Microsoft Corporation." MIT licence (full text below).

### GeoAnalystBench: Apache-2.0
- Source: https://github.com/GeoDS/GeoAnalystBench, commit b5d8c40a8d23639ec77e9acb11f79fd033c07338 (2026-02-11),
  `dataset/GeoAnalystBench.csv`.
- Paper: Zhang et al., *GeoAnalystBench*, arXiv 2509.05881.
- Changes: the Python workflow tasks were rewritten as plan-level requests in our contexts and
  classified.

### GeoAgentBench (pothos-dev): Apache-2.0
- Source: https://github.com/pothos-dev/geoagentbench, commit 1cd6f83b59c99048b3094f454de4a40b5ab4f749 (2026-06-18),
  `tasks/*/task.json` and READMEs.
- Changes: tasks were rewritten as plan-level requests in our contexts. None of the bundled
  data was used.
- NOTICE (reproduced as Apache-2.0 §4(d) requires):

  > GeoAgentBench
  > Copyright 2026 Daniel Allmer
  >
  > Licensed under the Apache License, Version 2.0 (the "License"); see the LICENSE file. This
  > license covers the original code and task definitions (graders, harness, eval, prompts, scripts).

  The NOTICE also says the bundled data (OpenStreetMap, ODbL 1.0; Overture, CDLA-Permissive-2.0
  and ODbL) keeps its own licences. We use none of that data.

### GeoAgentBench / GABench (Yu et al.): Apache-2.0
- Source: https://github.com/geox-lab/GABench, commit e8c64e883bbe45e2b94515c73941e7a0837320ae (2026-09-03),
  `benchmark/benchmark.csv` (57 rows).
- Paper: Yu et al., *GeoAgentBench: A Dynamic Execution Benchmark for Tool-Augmented Agents in
  Spatial Analysis*, arXiv 2604.13888.
- Changes: task descriptions were rewritten as plan-level requests in our contexts.

### BFCL v4 (irrelevance categories): Apache-2.0
- Source: https://github.com/ShishirPatil/gorilla, commit 6ea57973c7a6097fd7c5915698c54c17c5b1b6c8
  (2026-03-23), `berkeley-function-call-leaderboard/bfcl_eval/data/BFCL_v4_irrelevance.json` and
  the geographic items of `BFCL_v4_live_irrelevance.json`.
- Changes: questions are verbatim. The function lists were dropped and each question was placed
  in a LayeredMap context. Defective items are flagged in the records' notes.

### T-Eval: Apache-2.0
- Source: https://github.com/open-compass/T-Eval (commit 58f22406404d7e2a4f36856a19c7f4dc28a0a5f0) and
  https://huggingface.co/datasets/lovesnowbest/T-Eval (revision af355ab2b62cdbae4262ac41c7529ffeae395012,
  `data/plan_json_v2{,_zh}.json`).
- Changes: only items whose reference chains use BINGMap or GaodeMap are used. Their user
  queries are verbatim.

The Apache-2.0 sources are used under the Apache License, Version 2.0:
https://www.apache.org/licenses/LICENSE-2.0. Each record's `source` and `source_id` name the
original task, and its `text_policy` says whether we changed it.

## Sources used only as inspiration (no text copied)

| Source | Licence as found | What is in git |
|---|---|---|
| GTChain-Eval (https://github.com/AGI-GIS/GTChain, commit c77d8e00d53e40754b7f45417257f64cb18110c6) | Unclear: the LICENSE file is MIT, but the README says the data is "intended for research preview and non-commercial use only, subject to the model License of LLaMA2" | Summaries generated from each task's tool chain; our own requests |
| MapEval-API (https://github.com/MapEval/MapEval-API, commit 35d481ac797366592dd20f74bfbb75efe2961dc4) | None (no LICENSE file) | Summaries generated from each item's type fields; 73 original requests |
| SpatialQueryQA (https://github.com/Autonomous-GIS/Spatial-Text-to-SQL, commit 171dbce) | None (no LICENSE file) | Our own summaries and requests |
| GeoQuery (UT Austin ML group) | GPL-2.0 | 61 original questions of the same kinds; nothing copied |
| GIS Copilot (arXiv 2411.03205) | Paper; plugin code is GPL-3.0; the test-case repo is empty | Our own summaries and requests from the appendix task tables |
| GeoJSON Agent (arXiv 2509.08863) | Paper; no data released | Our own summaries and requests |
| ShapefileGPT (arXiv 2410.12376) | Paper; no data released | Our own summaries and requests |
| GeoLLM-Engine (arXiv 2404.15500) | Paper; no data released | Our own summaries and requests |

## MIT licence text (GeoBenchX, UnivEARTH, ACEBench)

Copyright (c) 2025 Varvara Krechetova (GeoBenchX)
Copyright (c) 2025 Chia-Hsiang Kao (UnivEARTH)
Copyright (c) Microsoft Corporation (ACEBench, as stated in its LICENSE.md)

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
associated documentation files (the "Software"), to deal in the Software without restriction,
including without limitation the rights to use, copy, modify, merge, publish, distribute,
sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or
substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT
OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
