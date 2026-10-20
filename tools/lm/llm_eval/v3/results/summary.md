# Harness v3 results

166 cases. Gold baseline: `mock.jsonl`.

| Run | e2e | honest | plan valid (1st try) | executes | answer | preview | area drawn | frames | median s | tokens in/out | cost |
|---|---|---|---|---|---|---|---|---|---|---|---|
| mock/mock | 145/166 (87%) | 100% | 100% (100%) | 100% | 100% | 84% | 91% | 38% | 0.02 | 0/0 | $0.000 |
| mock-prefix/mock | 94/166 (57%) | 100% | 100% (100%) | 100% | 100% | 45% | 91% | 0% | 0.02 | 0/0 | $0.000 |
| paid-main/anthropic_claude-haiku-5.5 | 124/166 (75%) | 98% | 99% (72%) | 99% | 81% | 73% | 22% | 29% | 3.13 | 4,462,803/54,023 | $0.161 |
| paid-main/z-ai_glm-5.3-flash | 96/166 (58%) | 95% | 74% (62%) | 74% | 59% | 69% | 28% | 22% | 6.23 | 3,230,557/153,684 | $0.260 |
| paid-norule19/anthropic_claude-haiku-5.5 | 114/166 (69%) | 95% | 97% (72%) | 97% | 77% | 70% | 24% | 31% | 3.04 | 4,621,923/55,197 | $0.169 |
| paid-ref/anthropic_claude-sonnet-5.5 | 133/166 (80%) | 99% | 100% (99%) | 100% | 96% | 79% | 69% | 37% | 5.39 | 3,542,171/68,916 | $2.515 |
| paid-repeat2/anthropic_claude-haiku-5.5 | 122/166 (73%) | 95% | 96% (76%) | 96% | 81% | 76% | 28% | 32% | 3.04 | 4,538,922/54,233 | $0.165 |
| replay-paid-main-prefix/anthropic_claude-haiku-5.5 | 75/166 (45%) | 98% | 89% (89%) | 89% | 69% | 38% | 22% | 0% | 0.03 | 0/0 | $0.000 |
| replay-paid-main-prefix/z-ai_glm-5.3-flash | 54/166 (33%) | 95% | 57% (57%) | 57% | 45% | 33% | 29% | 0% | 0.02 | 0/0 | $0.000 |
| replay-paid-norule19-prefix/anthropic_claude-haiku-5.5 | 72/166 (43%) | 95% | 93% (92%) | 93% | 70% | 38% | 24% | 0% | 0.02 | 0/0 | $0.000 |
| replay-paid-ref-prefix/anthropic_claude-sonnet-5.5 | 50/166 (30%) | 99% | 71% (71%) | 71% | 67% | 22% | 76% | 0% | 0.04 | 0/0 | $0.000 |
| replay-paid-repeat2-prefix/anthropic_claude-haiku-5.5 | 70/166 (42%) | 94% | 88% (88%) | 88% | 70% | 37% | 28% | 0% | 0.02 | 0/0 | $0.000 |

Failures by layer (first layer that failed):

| Run | model plan | checker | executor | preview-UI | prompt-tool gap | ground-truth ambiguity |
|---|---|---|---|---|---|---|
| mock/mock | 0 | 0 | 0 | 21 | 0 | 0 |
| mock-prefix/mock | 0 | 0 | 0 | 72 | 0 | 0 |
| paid-main/anthropic_claude-haiku-5.5 | 27 | 0 | 0 | 14 | 1 | 0 |
| paid-main/z-ai_glm-5.3-flash | 57 | 0 | 0 | 12 | 1 | 0 |
| paid-norule19/anthropic_claude-haiku-5.5 | 34 | 0 | 0 | 15 | 3 | 0 |
| paid-ref/anthropic_claude-sonnet-5.5 | 5 | 0 | 0 | 26 | 2 | 0 |
| paid-repeat2/anthropic_claude-haiku-5.5 | 28 | 0 | 0 | 12 | 4 | 0 |
| replay-paid-main-prefix/anthropic_claude-haiku-5.5 | 31 | 0 | 0 | 46 | 14 | 0 |
| replay-paid-main-prefix/z-ai_glm-5.3-flash | 54 | 0 | 0 | 33 | 25 | 0 |
| replay-paid-norule19-prefix/anthropic_claude-haiku-5.5 | 39 | 0 | 0 | 46 | 9 | 0 |
| replay-paid-ref-prefix/anthropic_claude-sonnet-5.5 | 5 | 0 | 0 | 68 | 43 | 0 |
| replay-paid-repeat2-prefix/anthropic_claude-haiku-5.5 | 31 | 0 | 0 | 49 | 16 | 0 |

End-to-end by question type:

| Run | chain | cities_near | honest | inside | join | nearest | outside | per_area | rank | raster | time | within |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| mock/mock | 17/17 | 6/10 | 21/21 | 12/12 | 6/6 | 4/14 | 2/4 | 15/15 | 5/5 | 10/10 | 7/7 | 40/45 |
| mock-prefix/mock | 16/17 | 6/10 | 21/21 | 0/12 | 6/6 | 4/14 | 2/4 | 15/15 | 5/5 | 10/10 | 7/7 | 2/45 |
| paid-main/anthropic_claude-haiku-5.5 | 16/17 | 2/10 | 20/21 | 5/12 | 6/6 | 3/14 | 2/4 | 15/15 | 5/5 | 6/10 | 7/7 | 37/45 |
| paid-main/z-ai_glm-5.3-flash | 12/17 | 3/10 | 21/21 | 3/12 | 6/6 | 2/14 | 1/4 | 11/15 | 3/5 | 5/10 | 5/7 | 24/45 |
| paid-norule19/anthropic_claude-haiku-5.5 | 15/17 | 1/10 | 19/21 | 4/12 | 6/6 | 1/14 | 2/4 | 15/15 | 5/5 | 5/10 | 4/7 | 37/45 |
| paid-ref/anthropic_claude-sonnet-5.5 | 16/17 | 1/10 | 21/21 | 11/12 | 6/6 | 4/14 | 1/4 | 15/15 | 5/5 | 7/10 | 6/7 | 40/45 |
| paid-repeat2/anthropic_claude-haiku-5.5 | 16/17 | 1/10 | 20/21 | 8/12 | 6/6 | 2/14 | 2/4 | 15/15 | 4/5 | 5/10 | 4/7 | 39/45 |
| replay-paid-main-prefix/anthropic_claude-haiku-5.5 | 8/17 | 2/10 | 20/21 | 3/12 | 6/6 | 2/14 | 2/4 | 11/15 | 4/5 | 6/10 | 7/7 | 4/45 |
| replay-paid-main-prefix/z-ai_glm-5.3-flash | 2/17 | 3/10 | 21/21 | 0/12 | 6/6 | 1/14 | 1/4 | 7/15 | 1/5 | 5/10 | 4/7 | 3/45 |
| replay-paid-norule19-prefix/anthropic_claude-haiku-5.5 | 8/17 | 1/10 | 19/21 | 3/12 | 6/6 | 1/14 | 2/4 | 15/15 | 5/5 | 5/10 | 4/7 | 3/45 |
| replay-paid-ref-prefix/anthropic_claude-sonnet-5.5 | 3/17 | 1/10 | 21/21 | 0/12 | 6/6 | 1/14 | 1/4 | 2/15 | 0/5 | 7/10 | 5/7 | 3/45 |
| replay-paid-repeat2-prefix/anthropic_claude-haiku-5.5 | 9/17 | 1/10 | 20/21 | 3/12 | 6/6 | 1/14 | 2/4 | 12/15 | 3/5 | 5/10 | 4/7 | 4/45 |

End-to-end by language:

| Run | de | en | es | fr | it | ja | ko | nl | pt | ru | zh |
|---|---|---|---|---|---|---|---|---|---|---|---|
| mock/mock | 2/3 | 79/88 | 5/6 | 4/5 | 1/1 | 6/7 | 1/1 | 2/2 | 1/1 | 1/1 | 43/51 |
| mock-prefix/mock | 1/3 | 58/88 | 1/6 | 2/5 | 0/1 | 5/7 | 0/1 | 1/2 | 0/1 | 0/1 | 26/51 |
| paid-main/anthropic_claude-haiku-5.5 | 2/3 | 69/88 | 4/6 | 4/5 | 0/1 | 4/7 | 1/1 | 2/2 | 1/1 | 1/1 | 36/51 |
| paid-main/z-ai_glm-5.3-flash | 2/3 | 52/88 | 3/6 | 3/5 | 1/1 | 3/7 | 1/1 | 1/2 | 1/1 | 0/1 | 29/51 |
| paid-norule19/anthropic_claude-haiku-5.5 | 2/3 | 63/88 | 4/6 | 3/5 | 0/1 | 4/7 | 1/1 | 2/2 | 1/1 | 1/1 | 33/51 |
| paid-ref/anthropic_claude-sonnet-5.5 | 2/3 | 72/88 | 5/6 | 3/5 | 1/1 | 5/7 | 1/1 | 2/2 | 1/1 | 1/1 | 40/51 |
| paid-repeat2/anthropic_claude-haiku-5.5 | 2/3 | 67/88 | 4/6 | 3/5 | 1/1 | 4/7 | 1/1 | 2/2 | 1/1 | 1/1 | 36/51 |
| replay-paid-main-prefix/anthropic_claude-haiku-5.5 | 1/3 | 45/88 | 2/6 | 3/5 | 0/1 | 2/7 | 0/1 | 0/2 | 0/1 | 0/1 | 22/51 |
| replay-paid-main-prefix/z-ai_glm-5.3-flash | 1/3 | 32/88 | 1/6 | 2/5 | 0/1 | 2/7 | 0/1 | 1/2 | 0/1 | 0/1 | 15/51 |
| replay-paid-norule19-prefix/anthropic_claude-haiku-5.5 | 1/3 | 45/88 | 1/6 | 2/5 | 0/1 | 2/7 | 0/1 | 1/2 | 0/1 | 0/1 | 20/51 |
| replay-paid-ref-prefix/anthropic_claude-sonnet-5.5 | 1/3 | 31/88 | 1/6 | 1/5 | 0/1 | 1/7 | 0/1 | 0/2 | 0/1 | 0/1 | 15/51 |
| replay-paid-repeat2-prefix/anthropic_claude-haiku-5.5 | 1/3 | 42/88 | 1/6 | 2/5 | 1/1 | 2/7 | 0/1 | 0/2 | 0/1 | 0/1 | 21/51 |
