# Harness v2 results

Rescored from the stored plans by `report_v2.py`. v1 rows: the 2026-10-17 v1 runs (v2 mode for them is the 78 corrected cases only; the v1 tools had no `stat`).

| Run | Model | v1 mode strict | v1 mode lenient | v2 strict | v2 lenient | Answerable strict | Honest refusal | US$/1k | US$/1k no cache | Cached | Median s | p90 s | Model calls | Repair rounds | Resolve calls |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| v1-per_action | anthropic/claude-haiku-5.5 | 88% | 94% | 87% | 92% | 86% (65) | 100% (8) | 0.790 | 0.790 | 0% | 2.23 | 3.07 | 1.86 | 0.00 | 0.00 |
| v1-per_action | google/gemini-3.5-flash | 72% | 92% | 72% | 92% | 72% (65) | 100% (8) | 4.977 | 4.977 | 0% | 2.84 | 4.28 | 2.05 | 0.00 | 0.00 |
| v1-per_action | openai/gpt-oss-120b | 74% | 88% | 74% | 88% | 71% (65) | 100% (8) | 0.079 | 0.249 | 74% | 4.67 | 8.57 | 1.96 | 0.00 | 0.00 |
| v1-per_action | z-ai/glm-5.3-flash | 79% | 85% | 79% | 85% | 75% (65) | 100% (8) | 0.445 | 0.494 | 56% | 2.13 | 6.87 | 2.08 | 0.00 | 0.00 |
| v1-per_action-repeat2 | anthropic/claude-haiku-5.5 | 81% | 86% | 81% | 85% | 80% (65) | 100% (8) | 0.779 | 0.779 | 0% | 2.22 | 3.31 | 1.82 | 0.00 | 0.00 |
| v1-per_action-repeat2 | openai/gpt-oss-120b | 72% | 81% | 72% | 79% | 68% (65) | 100% (8) | 0.069 | 0.213 | 74% | 4.35 | 6.41 | 1.69 | 0.00 | 0.00 |
| v1-per_action-repeat2 | z-ai/glm-5.3-flash | 77% | 85% | 77% | 85% | 75% (65) | 100% (8) | 0.323 | 0.394 | 76% | 3.00 | 6.44 | 2.19 | 0.00 | 0.00 |
| v2-per_action | anthropic/claude-haiku-5.5 | 97% | 97% | 99% | 99% | 99% (83) | 100% (8) | 0.348 | 1.698 | 94% | 2.49 | 3.82 | 2.18 | 0.01 | 0.29 |
| v2-per_action | google/gemini-3.5-flash | 99% | 99% | 98% | 98% | 98% (83) | 100% (8) | 6.827 | 13.693 | 55% | 11.21 | 16.78 | 2.36 | 0.02 | 0.33 |
| v2-per_action | openai/gpt-oss-120b | 86% | 86% | 86% | 86% | 84% (83) | 100% (8) | 0.163 | 0.756 | 90% | 4.63 | 7.82 | 2.12 | 0.03 | 0.25 |
| v2-per_action | z-ai/glm-5.3-flash | 99% | 99% | 99% | 99% | 99% (83) | 100% (8) | 0.984 | 1.134 | 77% | 2.70 | 5.97 | 2.28 | 0.02 | 0.21 |
| v2-per_action-jev | typesafe/jev-router | 99% | 99% | 100% | 100% | 100% (83) | 100% (8) | 1.197 | 1.197 | 79% | 4.95 | 12.00 | 2.25 | 0.01 | 0.23 |
| v2-per_action-nochecker | anthropic/claude-haiku-5.5 | 94% | 94% | 96% | 96% | 96% (83) | 88% (8) | 0.379 | 1.689 | 93% | 2.48 | 3.91 | 2.16 | 0.00 | 0.28 |
| v2-per_action-nochecker | openai/gpt-oss-120b | 78% | 88% | 70% | 90% | 66% (83) | 100% (8) | 0.263 | 0.551 | 42% | 5.28 | 9.88 | 2.24 | 0.00 | 0.27 |
| v2-per_action-nochecker | z-ai/glm-5.3-flash | 95% | 96% | 94% | 96% | 94% (83) | 100% (8) | 0.969 | 1.113 | 75% | 3.80 | 9.83 | 2.25 | 0.00 | 0.22 |
| v2-per_action-noexamples | anthropic/claude-haiku-5.5 | 97% | 97% | 99% | 99% | 99% (83) | 100% (8) | 0.362 | 1.544 | 92% | 2.67 | 4.88 | 2.21 | 0.01 | 0.28 |
| v2-per_action-noexamples | openai/gpt-oss-120b | 83% | 85% | 84% | 85% | 83% (83) | 100% (8) | 0.239 | 0.525 | 46% | 5.26 | 11.54 | 2.33 | 0.04 | 0.30 |
| v2-per_action-noexamples | z-ai/glm-5.3-flash | 95% | 95% | 96% | 96% | 96% (83) | 100% (8) | 0.886 | 1.009 | 72% | 2.77 | 5.44 | 2.27 | 0.04 | 0.19 |
| v2-per_action-noexamples-nochecker-noresolve | anthropic/claude-haiku-5.5 | 92% | 95% | 94% | 96% | 95% (83) | 100% (8) | 0.343 | 1.410 | 92% | 2.38 | 3.84 | 2.07 | 0.00 | 0.13 |
| v2-per_action-noresolve | anthropic/claude-haiku-5.5 | 94% | 94% | 94% | 94% | 94% (83) | 100% (8) | 0.355 | 1.537 | 92% | 2.38 | 3.91 | 2.01 | 0.00 | 0.12 |
| v2-per_action-repeat2 | anthropic/claude-haiku-5.5 | 99% | 99% | 100% | 100% | 100% (83) | 100% (8) | 0.356 | 1.725 | 94% | 2.38 | 3.98 | 2.21 | 0.01 | 0.29 |
| v2-per_action-repeat2 | openai/gpt-oss-120b | 86% | 86% | 87% | 87% | 86% (83) | 100% (8) | 0.169 | 0.833 | 94% | 4.66 | 9.46 | 2.28 | 0.07 | 0.27 |
| v2-per_action-repeat2 | z-ai/glm-5.3-flash | 97% | 97% | 99% | 99% | 99% (83) | 100% (8) | 0.979 | 1.128 | 77% | 2.81 | 6.86 | 2.27 | 0.02 | 0.21 |
| v2-submit_plan | anthropic/claude-haiku-5.5 | 96% | 96% | 98% | 98% | 98% (83) | 100% (8) | 0.206 | 0.720 | 90% | 1.34 | 2.54 | 1.29 | 0.03 | 0.28 |
| v2-submit_plan | openai/gpt-oss-120b | 87% | 87% | 90% | 90% | 89% (83) | 100% (8) | 0.087 | 0.321 | 81% | 3.55 | 7.20 | 1.24 | 0.00 | 0.24 |
| v2-submit_plan | z-ai/glm-5.3-flash | 99% | 99% | 100% | 100% | 100% (83) | 100% (8) | 0.380 | 0.431 | 74% | 1.23 | 2.58 | 1.22 | 0.00 | 0.24 |
| v2-submit_plan-jev | typesafe/jev-router | 96% | 96% | 98% | 98% | 98% (83) | 100% (8) | 0.445 | 0.445 | 67% | 2.20 | 4.17 | 1.24 | 0.00 | 0.26 |

## By category (v2 strict)

| Run | Model | action | filter | multi | zh | vague | impossible | trap | units | stat | resolve |
|---|---|---|---|---|---|---|---|---|---|---|---|
| v1-per_action | anthropic/claude-haiku-5.5 | 85% | 94% | 82% | 76% | 100% | 80% | 100% | 82% | 0% | – |
| v1-per_action | google/gemini-3.5-flash | 74% | 88% | 36% | 65% | 60% | 100% | 67% | 64% | 0% | – |
| v1-per_action | openai/gpt-oss-120b | 81% | 76% | 36% | 47% | 100% | 80% | 100% | 45% | 0% | – |
| v1-per_action | z-ai/glm-5.3-flash | 81% | 82% | 64% | 65% | 100% | 100% | 100% | 64% | 0% | – |
| v1-per_action-repeat2 | anthropic/claude-haiku-5.5 | 85% | 88% | 64% | 76% | 100% | 60% | 100% | 73% | 0% | – |
| v1-per_action-repeat2 | openai/gpt-oss-120b | 89% | 65% | 18% | 65% | 100% | 80% | 83% | 45% | 0% | – |
| v1-per_action-repeat2 | z-ai/glm-5.3-flash | 78% | 82% | 73% | 59% | 80% | 80% | 100% | 55% | 0% | – |
| v2-per_action | anthropic/claude-haiku-5.5 | 96% | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 100% |
| v2-per_action | google/gemini-3.5-flash | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 82% | 100% |
| v2-per_action | openai/gpt-oss-120b | 93% | 84% | 57% | 92% | 83% | 100% | 100% | 83% | 82% | 78% |
| v2-per_action | z-ai/glm-5.3-flash | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 91% | 100% |
| v2-per_action-jev | typesafe/jev-router | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 100% |
| v2-per_action-nochecker | anthropic/claude-haiku-5.5 | 93% | 100% | 100% | 100% | 100% | 100% | 83% | 83% | 100% | 100% |
| v2-per_action-nochecker | openai/gpt-oss-120b | 78% | 79% | 57% | 50% | 83% | 100% | 100% | 58% | 45% | 22% |
| v2-per_action-nochecker | z-ai/glm-5.3-flash | 96% | 100% | 93% | 92% | 83% | 100% | 100% | 92% | 91% | 78% |
| v2-per_action-noexamples | anthropic/claude-haiku-5.5 | 100% | 100% | 93% | 100% | 100% | 100% | 100% | 100% | 100% | 100% |
| v2-per_action-noexamples | openai/gpt-oss-120b | 93% | 89% | 57% | 77% | 67% | 100% | 100% | 83% | 82% | 78% |
| v2-per_action-noexamples | z-ai/glm-5.3-flash | 96% | 100% | 93% | 100% | 83% | 100% | 100% | 92% | 100% | 89% |
| v2-per_action-noexamples-nochecker-noresolve | anthropic/claude-haiku-5.5 | 96% | 95% | 86% | 88% | 67% | 100% | 100% | 92% | 100% | 89% |
| v2-per_action-noresolve | anthropic/claude-haiku-5.5 | 89% | 100% | 93% | 92% | 83% | 100% | 100% | 100% | 100% | 78% |
| v2-per_action-repeat2 | anthropic/claude-haiku-5.5 | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 100% |
| v2-per_action-repeat2 | openai/gpt-oss-120b | 96% | 84% | 57% | 88% | 83% | 100% | 100% | 83% | 91% | 78% |
| v2-per_action-repeat2 | z-ai/glm-5.3-flash | 96% | 100% | 100% | 100% | 100% | 100% | 100% | 92% | 100% | 100% |
| v2-submit_plan | anthropic/claude-haiku-5.5 | 100% | 95% | 93% | 100% | 100% | 100% | 100% | 100% | 100% | 100% |
| v2-submit_plan | openai/gpt-oss-120b | 89% | 89% | 71% | 92% | 83% | 100% | 100% | 83% | 100% | 78% |
| v2-submit_plan | z-ai/glm-5.3-flash | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 100% |
| v2-submit_plan-jev | typesafe/jev-router | 96% | 95% | 100% | 100% | 100% | 100% | 100% | 92% | 100% | 100% |

## Failures (v2 strict)

- **v2-per_action / anthropic/claude-haiku-5.5**: 1 — a08 (extra_steps 1); repaired cases 1, plans read from text 1, prose read as ask/refuse 0
- **v2-per_action / google/gemini-3.5-flash**: 2 — s04, s10 (extra_steps 2); repaired cases 2, plans read from text 0, prose read as ask/refuse 0
- **v2-per_action / openai/gpt-oss-120b**: 14 — a11, f07, f14, m01, m06, m07, m08, r08, r09, s03, s08, u03, v04, z01 (refused_answerable 7, wrong_action 3, asked_unnecessarily 2, missing_steps 1, acted_instead_of_asking 1); repaired cases 2, plans read from text 12, prose read as ask/refuse 0
- **v2-per_action / z-ai/glm-5.3-flash**: 1 — s06 (extra_steps 1); repaired cases 2, plans read from text 2, prose read as ask/refuse 0
- **v2-per_action-jev / typesafe/jev-router**: 0 —  (); repaired cases 1, plans read from text 1, prose read as ask/refuse 0
- **v2-per_action-nochecker / anthropic/claude-haiku-5.5**: 4 — a08, a11, t02, u04 (extra_steps 2, wrong_object 1, acted_instead_of_asking 1); repaired cases 0, plans read from text 2, prose read as ask/refuse 0
- **v2-per_action-nochecker / openai/gpt-oss-120b**: 29 — a02, a05, a11, a16, a19, f02, f15, m01, m05, m06, m07, m09, r01, r02, r04, r05, r06, r08, r09, s01, s03, s04, s05, s06, u02, u04, v04, x05, z01 (extra_steps 20, refused_answerable 4, wrong_object 2, wrong_action 1, missing_steps 1, acted_instead_of_asking 1); repaired cases 0, plans read from text 21, prose read as ask/refuse 0
- **v2-per_action-nochecker / z-ai/glm-5.3-flash**: 6 — m06, r03, r04, s06, u04, z01 (extra_steps 2, wrong_object 2, wrong_layer 1, acted_instead_of_asking 1); repaired cases 0, plans read from text 1, prose read as ask/refuse 0
- **v2-per_action-noexamples / anthropic/claude-haiku-5.5**: 1 — m06 (missing_steps 1); repaired cases 1, plans read from text 0, prose read as ask/refuse 0
- **v2-per_action-noexamples / openai/gpt-oss-120b**: 16 — a07, f03, f08, f09, m03, m05, m06, m09, r07, r09, s03, s08, v02, v04, z01, z03 (refused_answerable 4, asked_unnecessarily 4, missing_steps 3, wrong_action 2, acted_instead_of_asking 2, wrong_param 1); repaired cases 4, plans read from text 0, prose read as ask/refuse 0
- **v2-per_action-noexamples / z-ai/glm-5.3-flash**: 3 — a11, r09, v03 (extra_steps 2, acted_instead_of_asking 1); repaired cases 3, plans read from text 0, prose read as ask/refuse 0
- **v2-per_action-noexamples-nochecker-noresolve / anthropic/claude-haiku-5.5**: 6 — a16, f17, m05, m09, r03, v04 (acted_instead_of_asking 2, wrong_object 2, wrong_filter 1, wrong_field 1); repaired cases 0, plans read from text 0, prose read as ask/refuse 0
- **v2-per_action-noresolve / anthropic/claude-haiku-5.5**: 6 — a08, a15, m07, r03, r04, z01 (extra_steps 4, asked_unnecessarily 1, refused_instead_of_asking 1); repaired cases 0, plans read from text 0, prose read as ask/refuse 0
- **v2-per_action-repeat2 / anthropic/claude-haiku-5.5**: 0 —  (); repaired cases 1, plans read from text 0, prose read as ask/refuse 0
- **v2-per_action-repeat2 / openai/gpt-oss-120b**: 13 — a11, f07, f09, m01, m06, m07, m08, r08, r09, s08, u03, v04, z03 (refused_answerable 5, extra_steps 3, wrong_action 3, asked_unnecessarily 1, acted_instead_of_asking 1); repaired cases 5, plans read from text 17, prose read as ask/refuse 0
- **v2-per_action-repeat2 / z-ai/glm-5.3-flash**: 1 — a11 (extra_steps 1); repaired cases 2, plans read from text 3, prose read as ask/refuse 0
- **v2-submit_plan / anthropic/claude-haiku-5.5**: 2 — f13, m10 (missing_steps 1, refused_answerable 1); repaired cases 3, plans read from text 0, prose read as ask/refuse 0
- **v2-submit_plan / openai/gpt-oss-120b**: 10 — a11, a20, f13, m01, m03, r08, r09, u03, v04, z01 (refused_answerable 4, wrong_action 3, missing_steps 1, acted_instead_of_asking 1, extra_steps 1); repaired cases 0, plans read from text 0, prose read as ask/refuse 0
- **v2-submit_plan / z-ai/glm-5.3-flash**: 0 —  (); repaired cases 0, plans read from text 0, prose read as ask/refuse 0
- **v2-submit_plan-jev / typesafe/jev-router**: 2 — a11, f13 (extra_steps 1, refused_answerable 1); repaired cases 0, plans read from text 0, prose read as ask/refuse 0
