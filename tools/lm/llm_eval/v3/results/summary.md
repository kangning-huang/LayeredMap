# Harness v3 results

166 cases. Gold baseline: `mock.jsonl`.

| Run | e2e | honest | plan valid (1st try) | executes | answer | preview | area drawn | frames | median s | tokens in/out | cost |
|---|---|---|---|---|---|---|---|---|---|---|---|
| mock/mock | 140/166 (84%) | 99% | 99% (99%) | 99% | 99% | 81% | 91% | 37% | 0.02 | 0/0 | $0.000 |
| mock-harnessfix/mock | 166/166 (100%) | 100% | 100% (100%) | 100% | 100% | 100% | 91% | 68% | 0.02 | 0/0 | $0.000 |
| mock-prefix/mock | 89/166 (54%) | 99% | 99% (99%) | 99% | 99% | 42% | 91% | 0% | 0.02 | 0/0 | $0.000 |
| paid-harnessfix/anthropic_claude-haiku-5.5 | 131/166 (79%) | 98% | 99% (74%) | 99% | 83% | 79% | 18% | 39% | 3.19 | 4,754,971/56,657 | $0.171 |
| paid-harnessfix/z-ai_glm-5.3-flash | 126/166 (76%) | 98% | 96% (79%) | 96% | 76% | 80% | 28% | 41% | 2.66 | 3,414,448/39,571 | $0.204 |
| paid-main/anthropic_claude-haiku-5.5 | 121/166 (73%) | 98% | 99% (72%) | 99% | 81% | 69% | 22% | 28% | 3.13 | 4,462,803/54,023 | $0.161 |
| paid-main/z-ai_glm-5.3-flash | 93/166 (56%) | 95% | 73% (62%) | 73% | 59% | 66% | 28% | 22% | 6.23 | 3,230,557/153,684 | $0.260 |
| paid-norule19/anthropic_claude-haiku-5.5 | 113/166 (68%) | 96% | 97% (71%) | 97% | 77% | 67% | 24% | 31% | 3.04 | 4,621,923/55,197 | $0.169 |
| paid-ref/anthropic_claude-sonnet-5.5 | 129/166 (78%) | 99% | 100% (99%) | 100% | 96% | 75% | 69% | 37% | 5.39 | 3,542,171/68,916 | $2.515 |
| paid-repeat2/anthropic_claude-haiku-5.5 | 120/166 (72%) | 96% | 96% (76%) | 96% | 80% | 73% | 28% | 32% | 3.04 | 4,538,922/54,233 | $0.165 |
| replay-paid-harnessfix-final/anthropic_claude-haiku-5.5 | 134/166 (81%) | 98% | 99% (99%) | 99% | 83% | 81% | 18% | 57% | 0.02 | 0/0 | $0.000 |
| replay-paid-harnessfix-final/z-ai_glm-5.3-flash | 132/166 (80%) | 98% | 96% (96%) | 96% | 76% | 85% | 28% | 61% | 0.02 | 0/0 | $0.000 |
| replay-paid-main-harnessfix/anthropic_claude-haiku-5.5 | 134/166 (81%) | 98% | 99% (99%) | 99% | 82% | 80% | 22% | 53% | 0.02 | 0/0 | $0.000 |
| replay-paid-main-harnessfix/z-ai_glm-5.3-flash | 107/166 (64%) | 95% | 74% (70%) | 74% | 60% | 80% | 28% | 47% | 0.02 | 0/0 | $0.000 |
| replay-paid-main-prefix/anthropic_claude-haiku-5.5 | 73/166 (44%) | 98% | 89% (89%) | 89% | 69% | 35% | 22% | 0% | 0.03 | 0/0 | $0.000 |
| replay-paid-main-prefix/z-ai_glm-5.3-flash | 53/166 (32%) | 95% | 57% (57%) | 57% | 45% | 32% | 29% | 0% | 0.02 | 0/0 | $0.000 |
| replay-paid-norule19-prefix/anthropic_claude-haiku-5.5 | 71/166 (43%) | 95% | 93% (92%) | 93% | 69% | 36% | 24% | 0% | 0.02 | 0/0 | $0.000 |
| replay-paid-ref-prefix/anthropic_claude-sonnet-5.5 | 49/166 (30%) | 99% | 70% (70%) | 70% | 67% | 21% | 76% | 0% | 0.04 | 0/0 | $0.000 |
| replay-paid-repeat2-prefix/anthropic_claude-haiku-5.5 | 69/166 (42%) | 95% | 87% (87%) | 87% | 70% | 35% | 28% | 0% | 0.02 | 0/0 | $0.000 |

Failures by layer (first layer that failed):

| Run | model plan | checker | executor | preview-UI | prompt-tool gap | ground-truth ambiguity |
|---|---|---|---|---|---|---|
| mock/mock | 1 | 0 | 0 | 25 | 0 | 0 |
| mock-harnessfix/mock | 0 | 0 | 0 | 0 | 0 | 0 |
| mock-prefix/mock | 1 | 0 | 0 | 76 | 0 | 0 |
| paid-harnessfix/anthropic_claude-haiku-5.5 | 25 | 0 | 0 | 9 | 1 | 0 |
| paid-harnessfix/z-ai_glm-5.3-flash | 31 | 0 | 0 | 6 | 3 | 0 |
| paid-main/anthropic_claude-haiku-5.5 | 28 | 0 | 0 | 17 | 0 | 0 |
| paid-main/z-ai_glm-5.3-flash | 58 | 0 | 0 | 14 | 1 | 0 |
| paid-norule19/anthropic_claude-haiku-5.5 | 35 | 0 | 0 | 16 | 2 | 0 |
| paid-ref/anthropic_claude-sonnet-5.5 | 6 | 0 | 0 | 30 | 1 | 0 |
| paid-repeat2/anthropic_claude-haiku-5.5 | 29 | 0 | 0 | 14 | 3 | 0 |
| replay-paid-harnessfix-final/anthropic_claude-haiku-5.5 | 25 | 0 | 0 | 6 | 1 | 0 |
| replay-paid-harnessfix-final/z-ai_glm-5.3-flash | 31 | 0 | 0 | 0 | 3 | 0 |
| replay-paid-main-harnessfix/anthropic_claude-haiku-5.5 | 27 | 0 | 0 | 5 | 0 | 0 |
| replay-paid-main-harnessfix/z-ai_glm-5.3-flash | 57 | 0 | 0 | 1 | 1 | 0 |
| replay-paid-main-prefix/anthropic_claude-haiku-5.5 | 31 | 0 | 0 | 48 | 14 | 0 |
| replay-paid-main-prefix/z-ai_glm-5.3-flash | 54 | 0 | 0 | 34 | 25 | 0 |
| replay-paid-norule19-prefix/anthropic_claude-haiku-5.5 | 40 | 0 | 0 | 47 | 8 | 0 |
| replay-paid-ref-prefix/anthropic_claude-sonnet-5.5 | 5 | 0 | 0 | 69 | 43 | 0 |
| replay-paid-repeat2-prefix/anthropic_claude-haiku-5.5 | 31 | 0 | 0 | 50 | 16 | 0 |

End-to-end by question type:

| Run | chain | cities_near | honest | inside | join | nearest | outside | per_area | rank | raster | time | within |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| mock/mock | 16/17 | 6/10 | 21/21 | 12/12 | 6/6 | 0/14 | 2/4 | 15/15 | 5/5 | 10/10 | 7/7 | 40/45 |
| mock-harnessfix/mock | 17/17 | 10/10 | 21/21 | 12/12 | 6/6 | 14/14 | 4/4 | 15/15 | 5/5 | 10/10 | 7/7 | 45/45 |
| mock-prefix/mock | 15/17 | 6/10 | 21/21 | 0/12 | 6/6 | 0/14 | 2/4 | 15/15 | 5/5 | 10/10 | 7/7 | 2/45 |
| paid-harnessfix/anthropic_claude-haiku-5.5 | 15/17 | 6/10 | 20/21 | 4/12 | 6/6 | 3/14 | 4/4 | 15/15 | 5/5 | 5/10 | 6/7 | 42/45 |
| paid-harnessfix/z-ai_glm-5.3-flash | 13/17 | 8/10 | 21/21 | 10/12 | 2/6 | 1/14 | 2/4 | 14/15 | 3/5 | 5/10 | 6/7 | 41/45 |
| paid-main/anthropic_claude-haiku-5.5 | 16/17 | 2/10 | 20/21 | 5/12 | 6/6 | 0/14 | 2/4 | 15/15 | 5/5 | 6/10 | 7/7 | 37/45 |
| paid-main/z-ai_glm-5.3-flash | 11/17 | 3/10 | 21/21 | 3/12 | 6/6 | 0/14 | 1/4 | 11/15 | 3/5 | 5/10 | 5/7 | 24/45 |
| paid-norule19/anthropic_claude-haiku-5.5 | 15/17 | 1/10 | 19/21 | 4/12 | 6/6 | 0/14 | 2/4 | 15/15 | 5/5 | 5/10 | 4/7 | 37/45 |
| paid-ref/anthropic_claude-sonnet-5.5 | 16/17 | 1/10 | 21/21 | 11/12 | 6/6 | 0/14 | 1/4 | 15/15 | 5/5 | 7/10 | 6/7 | 40/45 |
| paid-repeat2/anthropic_claude-haiku-5.5 | 16/17 | 1/10 | 20/21 | 8/12 | 6/6 | 0/14 | 2/4 | 15/15 | 4/5 | 5/10 | 4/7 | 39/45 |
| replay-paid-harnessfix-final/anthropic_claude-haiku-5.5 | 15/17 | 6/10 | 20/21 | 4/12 | 6/6 | 6/14 | 4/4 | 15/15 | 5/5 | 5/10 | 6/7 | 42/45 |
| replay-paid-harnessfix-final/z-ai_glm-5.3-flash | 13/17 | 8/10 | 21/21 | 10/12 | 2/6 | 7/14 | 2/4 | 14/15 | 3/5 | 5/10 | 6/7 | 41/45 |
| replay-paid-main-harnessfix/anthropic_claude-haiku-5.5 | 17/17 | 7/10 | 20/21 | 5/12 | 6/6 | 2/14 | 4/4 | 15/15 | 5/5 | 6/10 | 7/7 | 40/45 |
| replay-paid-main-harnessfix/z-ai_glm-5.3-flash | 12/17 | 7/10 | 21/21 | 3/12 | 6/6 | 5/14 | 1/4 | 11/15 | 3/5 | 5/10 | 5/7 | 28/45 |
| replay-paid-main-prefix/anthropic_claude-haiku-5.5 | 8/17 | 2/10 | 20/21 | 3/12 | 6/6 | 0/14 | 2/4 | 11/15 | 4/5 | 6/10 | 7/7 | 4/45 |
| replay-paid-main-prefix/z-ai_glm-5.3-flash | 2/17 | 3/10 | 21/21 | 0/12 | 6/6 | 0/14 | 1/4 | 7/15 | 1/5 | 5/10 | 4/7 | 3/45 |
| replay-paid-norule19-prefix/anthropic_claude-haiku-5.5 | 8/17 | 1/10 | 19/21 | 3/12 | 6/6 | 0/14 | 2/4 | 15/15 | 5/5 | 5/10 | 4/7 | 3/45 |
| replay-paid-ref-prefix/anthropic_claude-sonnet-5.5 | 3/17 | 1/10 | 21/21 | 0/12 | 6/6 | 0/14 | 1/4 | 2/15 | 0/5 | 7/10 | 5/7 | 3/45 |
| replay-paid-repeat2-prefix/anthropic_claude-haiku-5.5 | 9/17 | 1/10 | 20/21 | 3/12 | 6/6 | 0/14 | 2/4 | 12/15 | 3/5 | 5/10 | 4/7 | 4/45 |

End-to-end by language:

| Run | de | en | es | fr | it | ja | ko | nl | pt | ru | zh |
|---|---|---|---|---|---|---|---|---|---|---|---|
| mock/mock | 2/3 | 76/88 | 5/6 | 4/5 | 1/1 | 5/7 | 1/1 | 2/2 | 1/1 | 1/1 | 42/51 |
| mock-harnessfix/mock | 3/3 | 88/88 | 6/6 | 5/5 | 1/1 | 7/7 | 1/1 | 2/2 | 1/1 | 1/1 | 51/51 |
| mock-prefix/mock | 1/3 | 55/88 | 1/6 | 2/5 | 0/1 | 4/7 | 0/1 | 1/2 | 0/1 | 0/1 | 25/51 |
| paid-harnessfix/anthropic_claude-haiku-5.5 | 3/3 | 70/88 | 4/6 | 4/5 | 0/1 | 5/7 | 1/1 | 2/2 | 1/1 | 1/1 | 40/51 |
| paid-harnessfix/z-ai_glm-5.3-flash | 3/3 | 69/88 | 5/6 | 2/5 | 1/1 | 4/7 | 1/1 | 2/2 | 1/1 | 1/1 | 37/51 |
| paid-main/anthropic_claude-haiku-5.5 | 2/3 | 67/88 | 4/6 | 4/5 | 0/1 | 4/7 | 1/1 | 2/2 | 1/1 | 1/1 | 35/51 |
| paid-main/z-ai_glm-5.3-flash | 2/3 | 50/88 | 3/6 | 3/5 | 1/1 | 2/7 | 1/1 | 1/2 | 1/1 | 0/1 | 29/51 |
| paid-norule19/anthropic_claude-haiku-5.5 | 2/3 | 62/88 | 4/6 | 3/5 | 0/1 | 4/7 | 1/1 | 2/2 | 1/1 | 1/1 | 33/51 |
| paid-ref/anthropic_claude-sonnet-5.5 | 2/3 | 70/88 | 5/6 | 3/5 | 1/1 | 4/7 | 1/1 | 2/2 | 1/1 | 1/1 | 39/51 |
| paid-repeat2/anthropic_claude-haiku-5.5 | 2/3 | 66/88 | 4/6 | 3/5 | 1/1 | 4/7 | 1/1 | 2/2 | 1/1 | 1/1 | 35/51 |
| replay-paid-harnessfix-final/anthropic_claude-haiku-5.5 | 3/3 | 70/88 | 5/6 | 4/5 | 0/1 | 5/7 | 1/1 | 2/2 | 1/1 | 1/1 | 42/51 |
| replay-paid-harnessfix-final/z-ai_glm-5.3-flash | 3/3 | 72/88 | 5/6 | 3/5 | 1/1 | 5/7 | 1/1 | 2/2 | 1/1 | 1/1 | 38/51 |
| replay-paid-main-harnessfix/anthropic_claude-haiku-5.5 | 2/3 | 75/88 | 4/6 | 4/5 | 0/1 | 5/7 | 1/1 | 2/2 | 1/1 | 1/1 | 39/51 |
| replay-paid-main-harnessfix/z-ai_glm-5.3-flash | 2/3 | 57/88 | 3/6 | 4/5 | 1/1 | 3/7 | 1/1 | 1/2 | 1/1 | 0/1 | 34/51 |
| replay-paid-main-prefix/anthropic_claude-haiku-5.5 | 1/3 | 44/88 | 2/6 | 3/5 | 0/1 | 2/7 | 0/1 | 0/2 | 0/1 | 0/1 | 21/51 |
| replay-paid-main-prefix/z-ai_glm-5.3-flash | 1/3 | 31/88 | 1/6 | 2/5 | 0/1 | 2/7 | 0/1 | 1/2 | 0/1 | 0/1 | 15/51 |
| replay-paid-norule19-prefix/anthropic_claude-haiku-5.5 | 1/3 | 44/88 | 1/6 | 2/5 | 0/1 | 2/7 | 0/1 | 1/2 | 0/1 | 0/1 | 20/51 |
| replay-paid-ref-prefix/anthropic_claude-sonnet-5.5 | 1/3 | 31/88 | 1/6 | 1/5 | 0/1 | 1/7 | 0/1 | 0/2 | 0/1 | 0/1 | 14/51 |
| replay-paid-repeat2-prefix/anthropic_claude-haiku-5.5 | 1/3 | 42/88 | 1/6 | 2/5 | 1/1 | 2/7 | 0/1 | 0/2 | 0/1 | 0/1 | 20/51 |
