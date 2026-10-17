# LayeredMap Pro: prior work on LLM harnesses for GIS tool calling

*Web research as of 2026-10-17. Accuracy figures are self-reported by the authors unless noted. None of them are directly comparable to our 78-case strict metric.*

## 1. Relevant projects

| Project | Approach | Reported accuracy | License | What to borrow |
|---|---|---|---|---|
| **GeoBenchX** ([paper](https://arxiv.org/html/2503.18129v2), [repo](https://github.com/Solirinai/GeoBenchX)) | LangGraph ReAct agent with 23 GIS tools plus a `reject_task` tool. System prompt plus a "rules" prompt (when to reject, default year). No solver few-shot. LLM-as-judge scoring. | Solvable tasks: Sonnet 4 55%, o4-mini 51%, Haiku 3.5 40%. Unsolvable tasks correctly rejected: o4-mini 90%, Sonnet 4 17% | MIT | Closest analogue to us (`reject_task` ≈ `cannot_do`). Its error taxonomy matches ours: "relying on internal knowledge of names instead of checking dataset values" and "hallucinating unjustified processing steps". Reuse its task design and its separate solvable/unsolvable scoring. |
| **GIS Copilot** (Penn State) ([paper](https://arxiv.org/abs/2411.03205), [plugin](https://giscience.psu.edu/get-started-with-gis-copilot)) | QGIS plugin. The agent gets documentation for the key tools and parameters, then generates the workflow and code. | 100+ tasks in 3 tiers. Success is "high" on basic and intermediate tasks and weak on advanced ones. No headline number. | QGIS plugin | Per-tool documentation cards. Tiered eval design. |
| **LLM-Geo / Autonomous GIS** ([repo](https://github.com/gladcolor/LLM-Geo), [paper](https://pure.psu.edu/en/publications/autonomous-gis-the-next-generation-ai-powered-gis/)) | GPT-4 writes a workflow graph, then code for each step, then assembles and runs it. | About 80% of generated programs run ([lab page](https://giscience.psu.edu/llm-geo-an-open-source-autonomous-gis-prototype)) | GPL-3.0 | The idea of planning a workflow graph first. The GPL code is not reusable in a proprietary product. |
| **ShapefileGPT** ([paper](https://arxiv.org/html/2410.12376v2)) | A planner agent and a worker agent over 27 shapefile functions. | 95.24% task success on a textbook-derived benchmark | Paper only | Split planning from execution. The function catalogue is a reference. |
| **GeoJSON Agent** ([paper](https://arxiv.org/pdf/2509.08863v2)) | Compares function calling against code generation. | Function calling 85.71%, code generation 97.14% on 70 tasks. Function calling was "more stable". | Paper only | Evidence that our certified-action design trades some coverage for stability. |
| **GeoAgentBench / Plan-and-React** ([paper](https://arxiv.org/abs/2604.13888)) | 117 tools, 53 tasks. Separates global planning from reactive execution. | Top models below 45% parameter execution accuracy ([PDF](https://arxiv.org/pdf/2604.13888)) | Paper (several repos) | Its parameter-accuracy metric, which is analogous to our strict score. |
| **GISclaw** ([paper](https://arxiv.org/abs/2603.26845), [repo](https://github.com/geumjin99/GISclaw)) | Python sandbox. Prompt rules for schema analysis and domain knowledge. An error-memory module. | 97% mean success on GeoAnalystBench. Single-agent ReAct beat plan-execute-replan on every cloud model. | AGPL-3.0 | Schema-analysis prompting and error memory. Its plan-vs-ReAct result is a caveat for us. |
| **GeoAgent** ([paper](https://arxiv.org/html/2410.18792v1)) | Code generation with retrieval and static analysis inside Monte Carlo tree search. | Beats baselines on GeoCode | Paper only | Static checks before execution, which is our validator idea. |
| **GeoGPT** ([paper](https://alphaxiv.org/abs/2307.07930)) | Agent built on predefined GIS tools, aimed at non-professional users. | Case studies only | — | Framing for non-expert users. |
| **GeoTP / GTChain-Eval** ([paper](https://www.maxapress.com/article/doi/10.48130/ker-0026-0013)) | LLaMA-3-8B fine-tuned with LoRA on GIS tool-chain instructions (GeoITC). | Beats baselines on tool-chain and parameter accuracy | Paper only | Its metrics, tool-chain accuracy and parameter accuracy. A fine-tuning path if we ever self-host. |
| **GeoLLM-Engine** ([paper](https://arxiv.org/abs/2404.15500v1)) | 175+ tools, about 500k generated tasks checked by a model-based correctness checker. | — | Paper only | Pipeline for generating synthetic tasks. |
| **UnivEARTH** (Earth Engine) ([paper](https://arxiv.org/html/2504.12110v2)) | Agents write Earth Engine API code. | 33–40% accuracy. Code fails to run in 44–58% of attempts. | — | Evidence that free-form code is fragile, which supports certified actions. |
| **Esri ArcGIS Pro assistant** ([ArcNews](https://www.esri.com/about/newsroom/arcnews/the-next-era-of-ai-and-arcgis)) | Mostly beta. Natural language to SQL/Cypher plus documentation Q&A ([NSGIC deck](https://nsgic.org/wp-content/uploads/2025/11/2025-11-18-AI-in-ArcGIS-Update-for-NSGIC.pdf)). | None published | Proprietary | UX reference only. |
| **CARTO / Felt** ([CARTO](https://carto.com/blog/turning-claude-chatgpt-into-geospatial-agents/), [Felt MCP](https://www.felt.com/blog/introducing-felt-mcp-server)) | CARTO exposes versioned workflows as MCP tools rather than generating SQL. Felt offers about 30 MCP tools. | None published | Proprietary | CARTO's "explicit versioned workflows" matches our certified actions. |
| **QGIS MCP servers** ([jjsantos01](https://github.com/jjsantos01/qgis_mcp), [nkarasiak](https://github.com/nkarasiak/qgis-mcp)), **QGIS Claude Skill Package** ([repo](https://github.com/Impertio-Studio/QGIS-Claude-Skill-Package)) | MCP bridges into QGIS. 19 Claude Code skills. | None | jjsantos01: no license; nkarasiak: GPL-2.0; skills: MIT | The MIT skill texts are a source of tool-description wording. |

## 2. Benchmarks for calibrating our eval

- **GeoBenchX**: 202 tasks (123 solvable, 79 unsolvable) ([paper](https://arxiv.org/html/2503.18129v2)). It is the best template for adding unsolvable `cannot_do` and `ask_user` cases. Its MIT license allows adapting the tasks.
- **GeoAnalystBench**: 50 expert-validated tasks, scored on workflow validity ([paper](https://arxiv.org/pdf/2509.05881), [repo](https://github.com/GeoDS/GeoAnalystBench), Apache-2.0). It targets code generation, but its workflow-structure metrics transfer to plan grading.
- **GeoAgentBench** (parameter execution accuracy) and **GTChain-Eval** (tool-chain and parameter accuracy). Both are close to our strict metric ([1](https://arxiv.org/abs/2604.13888), [2](https://www.maxapress.com/article/doi/10.48130/ker-0026-0013)).
- **BFCL**: its irrelevance category (240 items) maps to our `cannot_do` cases ([evalscope](https://evalscope.readthedocs.io/en/latest/third_party/bfcl_v4.html)). Epoch AI found defects in 48% of sampled BFCL v4 items, so audit any borrowed cases ([review](https://epoch.ai/benchmarks/berkeley-function-calling-leaderboard/review)).
- **Chinese tool calling**: ACEBench (2,000 Chinese and English entries) ([listing](https://benchmarklist.com/benchmarks/acebench/)) and T-Eval (Chinese data and leaderboard) ([repo](https://github.com/open-compass/t-eval)).
- No public dataset maps bilingual natural-language GIS requests to tool calls. Ours may be novel and worth publishing.

## 3. Ranked techniques for LayeredMap

**1. Entity-resolution tool with a multilingual alias index.** Impact: high, because it targets the Chinese-name failures. Cost: low to medium.
- Add `resolve_feature(layer, query)`. It searches stored names and aliases locally and returns the top-k canonical stored names with scores. The model still never sees bulk feature values.
- Build the alias set from:
  - Wikidata zh/en labels and aliases ([Help:Label](https://www.wikidata.org/wiki/Help:Label))
  - GeoNames alternate names ([dump](https://download.geonames.org/export/dump/))
  - Traditional/Simplified conversion with OpenCC ([repo](https://github.com/BYVoid/OpenCC))
  - Pinyin keys with pypinyin ([repo](https://github.com/mozillazg/python-pypinyin))
- Do not rely on GeoNames alone for Chinese: in 2015 only 63% of features located in China had a Chinese name there ([Clinga, ISWC 2016](https://files.ifi.uzh.ch/ddis/iswc_archive/iswc/ab/2016/www-kasm.nii.ac.jp/iswc2016/papers/paper_R23_.pdf)).
- GeoBenchX lists name-guessing as a top failure ([paper](https://arxiv.org/html/2503.18129v2)). Anthropic reports that returning readable names instead of opaque IDs reduces hallucination ([blog](https://www.anthropic.com/engineering/writing-tools-for-agents)).
- When the match is ambiguous, route to `ask_user`.

**2. A single forced `submit_plan` tool with a typed `steps` array.** Impact: high for GPT-5-style prose plans and one-tool-per-turn behaviour. Cost: low.
- Force the call with `tool_choice`. Anthropic's `tool_choice` combined with `disable_parallel_tool_use` guarantees exactly one tool call ([docs](https://platform.claude.com/docs/en/agents-and-tools/tool-use/parallel-tool-use)).
- Planning all calls up front (ReWOO) used 64% fewer tokens and gained 4.4 accuracy points over ReAct ([paper](https://arxiv.org/pdf/2305.18323)).
- Caveat: GISclaw found ReAct beat plan-execute in code agents ([paper](https://arxiv.org/abs/2603.26845)). A/B test both on our 78 cases.

**3. Strict schemas with dynamic enums.** Impact: medium to high. Cost: low.
- Build enums for layer and field names from the open project at request time, so invalid names become impossible.
- Use per-action schemas in a discriminated union.
- OpenAI says to "use enums… to prevent invalid states" and to always enable strict mode ([guide](https://developers.openai.com/api/docs/guides/function-calling)). Its own eval showed schema adherence of 100% with structured outputs versus under 40% without ([OpenAI](https://openai.com/index/introducing-structured-outputs-in-the-api/)).
- Claude supports `strict: true` on tools ([docs](https://docs.claude.com/en/docs/build-with-claude/structured-outputs)).
- On OpenRouter, enforcement varies by provider. Set `require_parameters: true` ([docs](https://openrouter.ai/docs/features/structured-outputs)).

**4. Deterministic plan validator and repair loop (at most 2 rounds).** Impact: medium to high. Cost: low, plus one extra call only on failure.
- Check types, field kinds and units, raster versus vector inputs, step dependencies, and calls the user did not request (zoom, color).
- Return specific, fix-oriented errors. Anthropic recommends error messages that state the fix and include examples ([blog](https://www.anthropic.com/engineering/writing-tools-for-agents)).
- CRITIC: correction driven by external feedback gained 7–8 points, while self-correction alone gave little or even hurt ([paper](https://arxiv.org/pdf/2305.11738v2)).

**5. Retrieved few-shot examples plus `input_examples`.** Impact: high for Claude and small models. Cost: low.
- Retrieve 3 semantically similar solved cases, including Chinese ones, and insert them as messages.
- LangChain measured: Claude 3 Sonnet rose from 16% to 52%, and Claude 3 Haiku from 11% to 75%. Similar examples beat static ones, and 3 examples matched using all 13 ([blog](https://www.langchain.com/blog/few-shot-prompting-to-improve-tool-calling-performance)).
- Anthropic's `input_examples` raised accuracy on complex parameters from 72% to 90% ([blog](https://www.anthropic.com/engineering/advanced-tool-use)).
- Caveat: OpenAI warns that examples "may hurt performance for reasoning models" ([guide](https://developers.openai.com/api/docs/guides/function-calling)).

**6. Rewrite tool descriptions and add a "minimal plan" rule.** Impact: medium. Cost: low.
- Write each description "as you would to a new hire". State when and when not to use each tool, and say explicitly not to add display steps the user did not ask for.
- Have an agent review failed transcripts to propose description fixes. Anthropic credits this method with its SWE-bench gains ([blog](https://www.anthropic.com/engineering/writing-tools-for-agents)).

**7. Prompt caching.** Impact: cost only. Cost: trivial.
- Cache the static prefix (system prompt, tool schemas, fixed examples). Put retrieved examples and layer descriptions after the breakpoint.
- On OpenRouter, Anthropic cache reads cost 0.1x and writes 1.25x. The minimum cacheable prefix for Haiku 5.5 is 512 tokens. Sticky routing keeps the cache warm ([docs](https://openrouter.ai/docs/features/prompt-caching)).

**8. Self-consistency voting.** Impact: uncertain. Cost: N times the calls.
- The evidence comes from chain-of-thought answer voting ([Wang et al.](https://arxiv.org/abs/2203.11171)). I found no controlled study of voting over tool plans.
- Use it only as an ambiguity detector: if samples disagree, route to `ask_user`.

## 4. Caveats

- Most GIS-agent results come from the authors' own benchmarks, are judged by an LLM, and target code generation. None measure exact-match plans over a closed action set like ours.
- Model rankings change from one benchmark to another. For example, Sonnet 4 rejected only 17% of unsolvable tasks in GeoBenchX ([paper](https://arxiv.org/html/2503.18129v2)). Score `cannot_do` and `ask_user` separately.
- Vendor numbers (Anthropic, OpenAI, LangChain) come from internal evals on older models.
- Licenses: LLM-Geo is GPL-3.0, GISclaw is AGPL-3.0, and nkarasiak/qgis-mcp is GPL-2.0, so take ideas only. GeoBenchX (MIT), GeoAnalystBench (Apache-2.0) and the QGIS skills (MIT) can be adapted with attribution.
- With 78 cases, one case is about 1.3 points. Report confidence intervals and repeat runs, as GISclaw does with bootstrap CIs.
