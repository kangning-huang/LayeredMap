52 requests, 22 solved, US$0.0935 in all.

| Puzzle | Model | Lang | Solved | Outcomes | Steps | Cost US$ (all runs) | Seconds (mean) |
|---|---|---|---|---|---|---|---|
| P1 | claude-haiku-5.5 | en | 2/2 | solved, solved | 4/4 | 0.0024 | 5 |
| P1 | claude-haiku-5.5 | zh | 2/2 | solved, solved | 4/4 | 0.0027 | 6 |
| P1 | glm-5.3-flash | en | 2/2 | solved, solved | 4/4 | 0.0014 | 7 |
| P1 | glm-5.3-flash | zh | 2/2 | solved, solved | 4/4 | 0.0013 | 5 |
| P2 | claude-haiku-5.5 | en | 2/2 | solved, solved | 4/3 | 0.0020 | 3 |
| P2 | glm-5.3-flash | en | 2/2 | solved, solved | 4/4 | 0.0016 | 5 |
| P3 | claude-haiku-5.5 | en | 0/2 | empty, empty | 5/5 | 0.0032 | 17 |
| P3 | claude-haiku-5.5 | zh | 1/2 | empty, solved | 5/5 | 0.0032 | 11 |
| P3 | glm-5.3-flash | en | 0/2 | empty, empty | 6/6 | 0.0032 | 17 |
| P3 | glm-5.3-flash | zh | 1/2 | empty, solved | 9/10 | 0.0078 | 46 |
| P4 | claude-haiku-5.5 | en | 0/2 | wrong, area_only | 6/6 | 0.0034 | 5 |
| P4 | claude-haiku-5.5 | zh | 2/2 | solved, solved | 6/5 | 0.0024 | 5 |
| P4 | glm-5.3-flash | en | 0/2 | exec_error, exec_error | 4/3 | 0.0017 | 5 |
| P4 | glm-5.3-flash | zh | 0/2 | exec_error, exec_error | 3/3 | 0.0015 | 6 |
| P5 | claude-haiku-5.5 | en | 2/2 | solved, solved | 4/3 | 0.0026 | 4 |
| P5 | glm-5.3-flash | en | 2/2 | solved, solved | 4/5 | 0.0017 | 5 |
| P6 | claude-haiku-5.5 | en | 2/2 | solved, solved | 10/10 | 0.0027 | 6 |
| P6 | glm-5.3-flash | en | 0/2 | exec_error, exec_error | 10/7 | 0.0060 | 46 |
| P7 | claude-haiku-5.5 | en | 0/2 | ambiguous, refused | 6/1 | 0.0037 | 5 |
| P7 | claude-haiku-5.5 | zh | 0/2 | asked, refused | 1/1 | 0.0025 | 4 |
| P7 | glm-5.3-flash | en | 0/2 | wrong, no_plan | 12/0 | 0.0078 | 80 |
| P7 | glm-5.3-flash | zh | 0/2 | exec_error, asked | 9/1 | 0.0057 | 49 |
| P8 | claude-haiku-5.5 | en | 0/2 | empty, exec_error | 5/8 | 0.0034 | 10 |
| P8 | claude-haiku-5.5 | zh | 0/2 | wrong, wrong | 7/6 | 0.0022 | 12 |
| P8 | glm-5.3-flash | en | 0/2 | exec_error, exec_error | 8/8 | 0.0082 | 43 |
| P8 | glm-5.3-flash | zh | 0/2 | exec_error, exec_error | 7/11 | 0.0089 | 110 |

Every request:

| Puzzle | Model | Lang | Run | Outcome | Steps | Shown | Cost US$ | Model s | Total s |
|---|---|---|---|---|---|---|---|---|---|
| P1 | claude-haiku-5.5 | en | 1 | solved | 4 | 1 | 0.0016 | 4.1 | 5.5 |
| P1 | claude-haiku-5.5 | en | 2 | solved | 4 | 1 | 0.0008 | 3.6 | 4.7 |
| P1 | claude-haiku-5.5 | zh | 1 | solved | 4 | 1 | 0.0019 | 5.1 | 6.6 |
| P1 | claude-haiku-5.5 | zh | 2 | solved | 4 | 1 | 0.0008 | 4.1 | 5.2 |
| P1 | glm-5.3-flash | en | 1 | solved | 4 | 1 | 0.0008 | 11.9 | 12.3 |
| P1 | glm-5.3-flash | en | 2 | solved | 4 | 1 | 0.0006 | 1.9 | 2.4 |
| P1 | glm-5.3-flash | zh | 1 | solved | 4 | 1 | 0.0006 | 6.4 | 6.8 |
| P1 | glm-5.3-flash | zh | 2 | solved | 4 | 1 | 0.0006 | 2.0 | 2.5 |
| P2 | claude-haiku-5.5 | en | 1 | solved | 4 | 1 | 0.0014 | 2.8 | 3.4 |
| P2 | claude-haiku-5.5 | en | 2 | solved | 3 | 1 | 0.0006 | 2.3 | 2.9 |
| P2 | glm-5.3-flash | en | 1 | solved | 4 | 1 | 0.0009 | 6.6 | 7.1 |
| P2 | glm-5.3-flash | en | 2 | solved | 4 | 1 | 0.0007 | 2.3 | 2.8 |
| P3 | claude-haiku-5.5 | en | 1 | empty | 5 | 0 | 0.0023 | 6.6 | 7.6 |
| P3 | claude-haiku-5.5 | en | 2 | empty | 5 | 0 | 0.0009 | 4.1 | 26.4 |
| P3 | claude-haiku-5.5 | zh | 1 | empty | 5 | 0 | 0.0013 | 5.8 | 7.8 |
| P3 | claude-haiku-5.5 | zh | 2 | solved | 5 | 1 | 0.0019 | 8.6 | 14.9 |
| P3 | glm-5.3-flash | en | 1 | empty | 6 | 0 | 0.0024 | 24.9 | 26.8 |
| P3 | glm-5.3-flash | en | 2 | empty | 6 | 0 | 0.0009 | 5.4 | 7.3 |
| P3 | glm-5.3-flash | zh | 1 | empty | 9 | 0 | 0.0029 | 29.6 | 48.2 |
| P3 | glm-5.3-flash | zh | 2 | solved | 10 | 1 | 0.0049 | 43.0 | 44.3 |
| P4 | claude-haiku-5.5 | en | 1 | wrong | 6 | 400 | 0.0027 | 5.9 | 6.6 |
| P4 | claude-haiku-5.5 | en | 2 | area_only | 6 | 1 | 0.0007 | 2.6 | 3.3 |
| P4 | claude-haiku-5.5 | zh | 1 | solved | 6 | 1 | 0.0017 | 5.3 | 6.4 |
| P4 | claude-haiku-5.5 | zh | 2 | solved | 5 | 1 | 0.0007 | 2.9 | 3.6 |
| P4 | glm-5.3-flash | en | 1 | exec_error | 4 |  | 0.0010 | 5.2 | 5.8 |
| P4 | glm-5.3-flash | en | 2 | exec_error | 3 |  | 0.0007 | 2.9 | 3.5 |
| P4 | glm-5.3-flash | zh | 1 | exec_error | 3 |  | 0.0008 | 4.0 | 4.7 |
| P4 | glm-5.3-flash | zh | 2 | exec_error | 3 |  | 0.0007 | 5.9 | 6.5 |
| P5 | claude-haiku-5.5 | en | 1 | solved | 4 | 1 | 0.0015 | 2.6 | 3.1 |
| P5 | claude-haiku-5.5 | en | 2 | solved | 3 | 1 | 0.0012 | 3.8 | 4.6 |
| P5 | glm-5.3-flash | en | 1 | solved | 4 | 1 | 0.0009 | 3.0 | 3.5 |
| P5 | glm-5.3-flash | en | 2 | solved | 5 | 1 | 0.0009 | 5.6 | 6.1 |
| P6 | claude-haiku-5.5 | en | 1 | solved | 10 | 1 | 0.0017 | 4.2 | 5.9 |
| P6 | claude-haiku-5.5 | en | 2 | solved | 10 | 1 | 0.0009 | 4.1 | 6.0 |
| P6 | glm-5.3-flash | en | 1 | exec_error | 10 |  | 0.0034 | 25.6 | 27.3 |
| P6 | glm-5.3-flash | en | 2 | exec_error | 7 |  | 0.0026 | 31.1 | 64.3 |
| P7 | claude-haiku-5.5 | en | 1 | ambiguous | 6 | 9 | 0.0023 | 5.0 | 6.2 |
| P7 | claude-haiku-5.5 | en | 2 | refused | 1 |  | 0.0013 | 4.4 | 4.5 |
| P7 | claude-haiku-5.5 | zh | 1 | asked | 1 |  | 0.0012 | 3.0 | 3.0 |
| P7 | claude-haiku-5.5 | zh | 2 | refused | 1 |  | 0.0013 | 4.5 | 4.5 |
| P7 | glm-5.3-flash | en | 1 | wrong | 12 | 93 | 0.0038 | 52.7 | 60.6 |
| P7 | glm-5.3-flash | en | 2 | no_plan | 0 |  | 0.0040 | 99.0 | 99.0 |
| P7 | glm-5.3-flash | zh | 1 | exec_error | 9 |  | 0.0041 | 40.4 | 41.2 |
| P7 | glm-5.3-flash | zh | 2 | asked | 1 |  | 0.0015 | 41.9 | 57.6 |
| P8 | claude-haiku-5.5 | en | 1 | empty | 5 | 0 | 0.0016 | 5.6 | 7.1 |
| P8 | claude-haiku-5.5 | en | 2 | exec_error | 8 |  | 0.0018 | 10.6 | 12.1 |
| P8 | claude-haiku-5.5 | zh | 1 | wrong | 7 | 24 | 0.0010 | 6.8 | 13.6 |
| P8 | claude-haiku-5.5 | zh | 2 | wrong | 6 | 3 | 0.0012 | 7.6 | 10.5 |
| P8 | glm-5.3-flash | en | 1 | exec_error | 8 |  | 0.0039 | 26.0 | 32.4 |
| P8 | glm-5.3-flash | en | 2 | exec_error | 8 |  | 0.0043 | 44.0 | 53.1 |
| P8 | glm-5.3-flash | zh | 1 | exec_error | 7 |  | 0.0048 | 43.3 | 45.2 |
| P8 | glm-5.3-flash | zh | 2 | exec_error | 11 |  | 0.0041 | 41.2 | 174.6 |

