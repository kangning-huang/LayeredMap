## per_action

| Model | Strict | Lenient | $ / 1,000 req | Median s | p90 s | Prompt tok | Completion tok | Reasoning tok | Errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| anthropic/claude-haiku-5.5 * | 88% | 94% | 0.790 | 2.23 | 3.07 | 6868 | 222 | 0 | 0 |
| anthropic/claude-sonnet-5.5 | 88% | 95% | 16.251 | 2.77 | 4.84 | 7227 | 196 | 9 | 0 |
| z-ai/glm-5.3-flash * | 79% | 85% | 0.445 | 2.13 | 6.87 | 5104 | 122 | 37 | 0 |
| openai/gpt-oss-120b * | 74% | 88% | 0.079 | 4.67 | 8.57 | 3062 | 142 | 67 | 0 |
| google/gemini-3.5-flash | 72% | 92% | 4.977 | 2.84 | 4.28 | 2885 | 78 | 0 | 0 |
| openai/gpt-oss-20b * | 67% | 72% | 0.072 | 6.22 | 15.45 | 3019 | 138 | 24 | 0 |
| qwen/qwen3-235b-a22b-2507 | 54% | 85% | 0.516 | 3.62 | 7.41 | 6364 | 109 | 0 | 0 |
| deepseek/deepseek-v4-flash | 47% | 82% | 0.134 | 3.74 | 10.98 | 6992 | 222 | 0 | 0 |
| mistralai/mistral-small-3.2-24b-instruct | 47% | 77% | 0.452 | 1.81 | 4.08 | 5852 | 87 | 0 | 0 |
| meta-llama/llama-3.3-70b-instruct | 45% | 68% | 1.289 | 3.76 | 8.83 | 9767 | 95 | 0 | 0 |
| google/gemini-3.1-flash-lite | 37% | 74% | 1.281 | 3.00 | 5.05 | 4590 | 98 | 0 | 0 |
| openai/gpt-5-mini | 28% | 47% | 0.476 | 3.02 | 4.43 | 2187 | 161 | 0 | 0 |
| google/gemini-2.5-flash-lite | 23% | 38% | 0.167 | 1.55 | 4.85 | 830 | 273 | 253 | 0 |
| qwen/qwen3.7-flash | 22% | 72% | 0.190 | 5.27 | 8.31 | 10112 | 308 | 0 | 0 |
| openai/gpt-5-nano | 4% | 10% | 0.111 | 1.97 | 4.81 | 1960 | 225 | 0 | 0 |

\* Pareto-optimal on cost vs strict accuracy.

Strict accuracy by category:

| Model | action | filter | style | display | multi | zh | vague | impossible | trap | units |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| anthropic/claude-haiku-5.5 | 85% | 94% | 100% | 100% | 82% | 76% | 100% | 83% | 100% | 82% |
| anthropic/claude-sonnet-5.5 | 93% | 94% | 100% | 100% | 64% | 76% | 100% | 83% | 86% | 73% |
| z-ai/glm-5.3-flash | 81% | 82% | 100% | 50% | 64% | 65% | 100% | 83% | 86% | 64% |
| openai/gpt-oss-120b | 81% | 76% | 100% | 100% | 27% | 47% | 100% | 83% | 100% | 45% |
| google/gemini-3.5-flash | 74% | 88% | 100% | 100% | 36% | 65% | 60% | 83% | 57% | 64% |
| openai/gpt-oss-20b | 70% | 100% | 100% | 100% | 45% | 71% | 60% | 0% | 43% | 82% |
| qwen/qwen3-235b-a22b-2507 | 67% | 41% | 100% | 100% | 18% | 41% | 20% | 67% | 57% | 27% |
| deepseek/deepseek-v4-flash | 59% | 29% | 100% | 100% | 27% | 29% | 60% | 17% | 43% | 27% |
| mistralai/mistral-small-3.2-24b-instruct | 48% | 65% | 67% | 100% | 18% | 65% | 40% | 33% | 43% | 36% |
| meta-llama/llama-3.3-70b-instruct | 70% | 29% | 100% | 100% | 9% | 24% | 0% | 67% | 29% | 0% |
| google/gemini-3.1-flash-lite | 33% | 29% | 100% | 100% | 18% | 29% | 40% | 67% | 57% | 9% |
| openai/gpt-5-mini | 19% | 53% | 33% | 100% | 9% | 0% | 40% | 0% | 43% | 9% |
| google/gemini-2.5-flash-lite | 26% | 24% | 33% | 0% | 36% | 29% | 0% | 33% | 14% | 9% |
| qwen/qwen3.7-flash | 41% | 6% | 67% | 100% | 0% | 18% | 0% | 0% | 29% | 9% |
| openai/gpt-5-nano | 0% | 12% | 0% | 0% | 0% | 0% | 0% | 17% | 0% | 0% |

Strict failures by kind (all models):

| Kind | Count |
|---|---:|
| extra_steps | 217 |
| no_call | 117 |
| wrong_object | 40 |
| acted_instead_of_asking | 38 |
| asked_unnecessarily | 35 |
| malformed_call | 32 |
| acted_instead_of_refusing | 19 |
| wrong_action | 18 |
| wrong_layer | 14 |
| wrong_filter | 11 |
| missing_steps | 10 |
| asked_instead_of_refusing | 9 |
| refused_answerable | 3 |
| wrong_field | 1 |

Cases most often failed (strict):

| Case | Request | Models failing |
|---|---|---:|
| u02 | 以世纪大道为中心，缓冲半径 500 米 | 15 |
| a16 | 延安路有多长？ | 15 |
| m05 | 统计每个区的高中数量，然后按数量排名 | 15 |
| m09 | 世纪大道周围1公里内有哪些学校？ | 15 |
| a07 | Show me all the cities inside Brazil. | 13 |
| x05 | What is the total number of students in each district? | 13 |
| a18 | Show the land cover shares for each district. | 12 |
| u03 | Show the cities higher than 5,000 feet. | 12 |
| m04 | Which capital city sits highest above sea level? | 12 |
| m06 | Rank the African countries by their average elevation. | 12 |
| f04 | Show primary and middle schools. | 10 |
| a11 | Show everything within 100 miles of Paris. | 10 |
| m01 | How many cities above 1,000 m are there in each country? | 10 |
| t02 | Color the cities by continent. | 10 |
| v01 | Analyze the schools. | 10 |

## per_action-repeat2

| Model | Strict | Lenient | $ / 1,000 req | Median s | p90 s | Prompt tok | Completion tok | Reasoning tok | Errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| anthropic/claude-haiku-5.5 * | 81% | 86% | 0.779 | 2.22 | 3.31 | 6723 | 229 | 0 | 0 |
| z-ai/glm-5.3-flash * | 77% | 85% | 0.323 | 3.00 | 6.44 | 5477 | 188 | 90 | 0 |
| openai/gpt-oss-120b * | 72% | 81% | 0.069 | 4.35 | 6.41 | 2606 | 127 | 60 | 0 |

\* Pareto-optimal on cost vs strict accuracy.

Strict accuracy by category:

| Model | action | filter | style | display | multi | zh | vague | impossible | trap | units |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| anthropic/claude-haiku-5.5 | 85% | 88% | 100% | 100% | 64% | 76% | 100% | 50% | 86% | 73% |
| z-ai/glm-5.3-flash | 78% | 82% | 100% | 50% | 73% | 59% | 80% | 67% | 86% | 55% |
| openai/gpt-oss-120b | 89% | 65% | 100% | 100% | 18% | 65% | 100% | 67% | 71% | 45% |

Strict failures by kind (all models):

| Kind | Count |
|---|---:|
| no_call | 19 |
| extra_steps | 14 |
| wrong_object | 8 |
| wrong_filter | 4 |
| asked_unnecessarily | 3 |
| wrong_layer | 2 |
| wrong_action | 2 |
| asked_instead_of_refusing | 1 |
| acted_instead_of_refusing | 1 |
| acted_instead_of_asking | 1 |

Cases most often failed (strict):

| Case | Request | Models failing |
|---|---|---:|
| x05 | What is the total number of students in each district? | 3 |
| a07 | Show me all the cities inside Brazil. | 3 |
| m09 | 世纪大道周围1公里内有哪些学校？ | 3 |
| m05 | 统计每个区的高中数量，然后按数量排名 | 3 |
| u02 | 以世纪大道为中心，缓冲半径 500 米 | 3 |
| a16 | 延安路有多长？ | 3 |
| u04 | Draw 5 km around the selected city. | 2 |
| m06 | Rank the African countries by their average elevation. | 2 |
| z03 | 只显示首都 | 2 |
| a11 | Show everything within 100 miles of Paris. | 2 |
| x04 | Download the latest satellite images of Japan. | 2 |
| m04 | Which capital city sits highest above sea level? | 2 |
| a08 | Summarize the schools in the area I drew. | 1 |
| f04 | Show primary and middle schools. | 1 |
| f17 | Show countries whose population estimate is older than 2019. | 1 |

## per_action-with_values

| Model | Strict | Lenient | $ / 1,000 req | Median s | p90 s | Prompt tok | Completion tok | Reasoning tok | Errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| anthropic/claude-haiku-5.5 * | 87% | 94% | 0.768 | 2.26 | 2.99 | 6677 | 216 | 0 | 0 |
| z-ai/glm-5.3-flash * | 83% | 95% | 0.195 | 4.59 | 9.49 | 5246 | 374 | 292 | 0 |
| openai/gpt-oss-120b * | 72% | 78% | 0.072 | 4.30 | 6.94 | 2691 | 129 | 58 | 0 |
| openai/gpt-oss-20b | 68% | 82% | 0.091 | 2.53 | 4.50 | 3203 | 145 | 29 | 0 |

\* Pareto-optimal on cost vs strict accuracy.

Strict accuracy by category:

| Model | action | filter | style | display | multi | zh | vague | impossible | trap | units |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| anthropic/claude-haiku-5.5 | 89% | 100% | 100% | 100% | 73% | 76% | 100% | 67% | 100% | 73% |
| z-ai/glm-5.3-flash | 78% | 94% | 100% | 50% | 91% | 76% | 100% | 67% | 86% | 64% |
| openai/gpt-oss-120b | 89% | 65% | 67% | 100% | 18% | 53% | 80% | 83% | 71% | 45% |
| openai/gpt-oss-20b | 74% | 100% | 100% | 100% | 45% | 71% | 80% | 0% | 43% | 73% |

Strict failures by kind (all models):

| Kind | Count |
|---|---:|
| no_call | 28 |
| wrong_object | 18 |
| extra_steps | 9 |
| wrong_action | 4 |
| asked_unnecessarily | 3 |
| acted_instead_of_refusing | 2 |
| asked_instead_of_refusing | 2 |
| missing_steps | 2 |
| wrong_layer | 1 |
| acted_instead_of_asking | 1 |

Cases most often failed (strict):

| Case | Request | Models failing |
|---|---|---:|
| a16 | 延安路有多长？ | 4 |
| u02 | 以世纪大道为中心，缓冲半径 500 米 | 4 |
| m09 | 世纪大道周围1公里内有哪些学校？ | 4 |
| x04 | Download the latest satellite images of Japan. | 3 |
| u04 | Draw 5 km around the selected city. | 3 |
| m05 | 统计每个区的高中数量，然后按数量排名 | 3 |
| x05 | What is the total number of students in each district? | 3 |
| x02 | Forecast each country's population for 2050. | 2 |
| m10 | Color each country by how many cities it has. | 2 |
| m07 | Show 2 km around Century Ave and list the schools inside that zone. | 2 |
| a07 | Show me all the cities inside Brazil. | 2 |
| m03 | How many primary schools are there in each district? | 2 |
| a11 | Show everything within 100 miles of Paris. | 2 |
| m06 | Rank the African countries by their average elevation. | 2 |
| a21 | I want to draw an area and get the average elevation inside it. | 2 |

## single

| Model | Strict | Lenient | $ / 1,000 req | Median s | p90 s | Prompt tok | Completion tok | Reasoning tok | Errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| anthropic/claude-haiku-5.5 * | 76% | 79% | 0.615 | 2.34 | 3.37 | 4985 | 245 | 0 | 0 |
| openai/gpt-oss-120b * | 69% | 82% | 0.082 | 5.12 | 9.16 | 2806 | 154 | 77 | 0 |
| openai/gpt-oss-20b | 65% | 74% | 0.086 | 3.45 | 5.58 | 2808 | 142 | 27 | 1 |
| z-ai/glm-5.3-flash | 65% | 72% | 0.272 | 3.66 | 6.94 | 3721 | 207 | 119 | 0 |
| google/gemini-2.5-flash-lite | 53% | 67% | 0.320 | 2.46 | 7.28 | 1290 | 500 | 476 | 0 |
| deepseek/deepseek-v4-flash | 42% | 77% | 0.153 | 3.96 | 8.70 | 5647 | 245 | 0 | 0 |
| mistralai/mistral-small-3.2-24b-instruct | 42% | 54% | 0.258 | 2.01 | 3.97 | 3249 | 83 | 0 | 0 |
| openai/gpt-5-mini | 42% | 55% | 0.624 | 2.55 | 5.69 | 2526 | 182 | 0 | 0 |
| qwen/qwen3-235b-a22b-2507 | 33% | 55% | 0.402 | 3.86 | 9.32 | 4404 | 111 | 0 | 0 |
| qwen/qwen3.7-flash | 28% | 51% | 0.095 | 4.93 | 8.17 | 6866 | 209 | 0 | 0 |
| google/gemini-3.1-flash-lite | 21% | 62% | 1.391 | 4.42 | 5.44 | 4886 | 122 | 0 | 0 |
| openai/gpt-5-nano | 1% | 13% | 0.119 | 2.06 | 4.84 | 1850 | 232 | 0 | 0 |

\* Pareto-optimal on cost vs strict accuracy.

Strict accuracy by category:

| Model | action | filter | style | display | multi | zh | vague | impossible | trap | units |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| anthropic/claude-haiku-5.5 | 63% | 100% | 100% | 100% | 73% | 71% | 100% | 33% | 86% | 64% |
| openai/gpt-oss-120b | 67% | 59% | 100% | 100% | 45% | 59% | 80% | 100% | 86% | 64% |
| openai/gpt-oss-20b | 70% | 100% | 100% | 100% | 30% | 71% | 60% | 0% | 57% | 82% |
| z-ai/glm-5.3-flash | 59% | 76% | 100% | 50% | 36% | 53% | 100% | 67% | 86% | 36% |
| google/gemini-2.5-flash-lite | 30% | 94% | 100% | 100% | 27% | 41% | 0% | 67% | 86% | 55% |
| deepseek/deepseek-v4-flash | 48% | 18% | 67% | 100% | 45% | 29% | 60% | 17% | 43% | 27% |
| mistralai/mistral-small-3.2-24b-instruct | 33% | 82% | 33% | 50% | 9% | 47% | 40% | 50% | 43% | 45% |
| openai/gpt-5-mini | 44% | 59% | 67% | 100% | 18% | 6% | 40% | 17% | 29% | 27% |
| qwen/qwen3-235b-a22b-2507 | 19% | 53% | 100% | 100% | 9% | 24% | 20% | 50% | 43% | 27% |
| qwen/qwen3.7-flash | 30% | 35% | 67% | 100% | 0% | 18% | 0% | 33% | 29% | 27% |
| google/gemini-3.1-flash-lite | 11% | 12% | 100% | 50% | 9% | 18% | 40% | 67% | 14% | 9% |
| openai/gpt-5-nano | 0% | 0% | 0% | 50% | 0% | 0% | 0% | 0% | 0% | 0% |

Strict failures by kind (all models):

| Kind | Count |
|---|---:|
| extra_steps | 208 |
| no_call | 116 |
| wrong_object | 36 |
| wrong_action | 33 |
| acted_instead_of_asking | 28 |
| asked_unnecessarily | 23 |
| wrong_layer | 18 |
| acted_instead_of_refusing | 17 |
| asked_instead_of_refusing | 12 |
| missing_steps | 10 |
| wrong_filter | 7 |
| malformed_call | 5 |
| refused_answerable | 2 |

Cases most often failed (strict):

| Case | Request | Models failing |
|---|---|---:|
| a07 | Show me all the cities inside Brazil. | 12 |
| u02 | 以世纪大道为中心，缓冲半径 500 米 | 12 |
| a16 | 延安路有多长？ | 12 |
| m09 | 世纪大道周围1公里内有哪些学校？ | 12 |
| a11 | Show everything within 100 miles of Paris. | 11 |
| m05 | 统计每个区的高中数量，然后按数量排名 | 11 |
| z01 | 法国的面积是多少平方公里？ | 10 |
| m08 | Find the most populous capitals. | 10 |
| x05 | What is the total number of students in each district? | 10 |
| m07 | Show 2 km around Century Ave and list the schools inside that zone. | 10 |
| m04 | Which capital city sits highest above sea level? | 10 |
| a06 | What schools are inside this district? | 10 |
| a05 | Which country is Timbuktu in? | 9 |
| a20 | Mean elevation of Nepal? | 9 |
| a14 | What is the area of France? | 9 |

Total spent on 2026-10-17: US$2.7138
