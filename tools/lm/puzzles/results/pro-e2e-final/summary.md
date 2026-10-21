52 requests, 24 solved, US$0.0862 in all.

| Puzzle | Model | Lang | Solved | Outcomes | Steps | Cost US$ (all runs) | Seconds (mean) |
|---|---|---|---|---|---|---|---|
| P1 | claude-haiku-5.5 | en | 2/2 | solved, solved | 4/4 | 0.0016 | 5 |
| P1 | claude-haiku-5.5 | zh | 2/2 | solved, solved | 4/4 | 0.0026 | 7 |
| P1 | glm-5.3-flash | en | 2/2 | solved, solved | 4/4 | 0.0014 | 8 |
| P1 | glm-5.3-flash | zh | 2/2 | solved, solved | 4/4 | 0.0014 | 8 |
| P2 | claude-haiku-5.5 | en | 2/2 | solved, solved | 4/4 | 0.0013 | 3 |
| P2 | glm-5.3-flash | en | 2/2 | solved, solved | 5/4 | 0.0017 | 5 |
| P3 | claude-haiku-5.5 | en | 0/2 | empty, empty | 5/4 | 0.0024 | 27 |
| P3 | claude-haiku-5.5 | zh | 0/2 | wrong, asked | 5/1 | 0.0031 | 8 |
| P3 | glm-5.3-flash | en | 0/2 | empty, empty | 9/9 | 0.0036 | 9 |
| P3 | glm-5.3-flash | zh | 0/2 | empty, empty | 14/10 | 0.0051 | 23 |
| P4 | claude-haiku-5.5 | en | 1/2 | solved, wrong | 6/6 | 0.0026 | 7 |
| P4 | claude-haiku-5.5 | zh | 1/2 | exec_error, solved | 4/6 | 0.0025 | 5 |
| P4 | glm-5.3-flash | en | 1/2 | solved, exec_error | 5/4 | 0.0017 | 6 |
| P4 | glm-5.3-flash | zh | 0/2 | exec_error, exec_error | 3/3 | 0.0015 | 4 |
| P5 | claude-haiku-5.5 | en | 2/2 | solved, solved | 4/3 | 0.0019 | 5 |
| P5 | glm-5.3-flash | en | 2/2 | solved, solved | 5/4 | 0.0016 | 3 |
| P6 | claude-haiku-5.5 | en | 1/2 | solved, empty | 10/9 | 0.0035 | 9 |
| P6 | glm-5.3-flash | en | 2/2 | solved, solved | 10/9 | 0.0021 | 6 |
| P7 | claude-haiku-5.5 | en | 0/2 | refused, refused | 1/1 | 0.0022 | 4 |
| P7 | claude-haiku-5.5 | zh | 0/2 | asked, asked | 1/1 | 0.0024 | 3 |
| P7 | glm-5.3-flash | en | 0/2 | no_plan, no_plan | 0/0 | 0.0080 | 37 |
| P7 | glm-5.3-flash | zh | 0/2 | ambiguous, exec_error | 9/10 | 0.0068 | 26 |
| P8 | claude-haiku-5.5 | en | 0/2 | empty, asked | 5/1 | 0.0012 | 5 |
| P8 | claude-haiku-5.5 | zh | 0/2 | empty, ambiguous | 9/8 | 0.0035 | 12 |
| P8 | glm-5.3-flash | en | 1/2 | exec_error, solved | 17/16 | 0.0125 | 76 |
| P8 | glm-5.3-flash | zh | 1/2 | exec_error, solved | 11/8 | 0.0082 | 47 |

Every request:

| Puzzle | Model | Lang | Run | Outcome | Steps | Shown | Cost US$ | Model s | Total s |
|---|---|---|---|---|---|---|---|---|---|
| P1 | claude-haiku-5.5 | en | 1 | solved | 4 | 1 | 0.0008 | 3.9 | 5.5 |
| P1 | claude-haiku-5.5 | en | 2 | solved | 4 | 1 | 0.0008 | 4.0 | 5.3 |
| P1 | claude-haiku-5.5 | zh | 1 | solved | 4 | 1 | 0.0011 | 5.6 | 7.3 |
| P1 | claude-haiku-5.5 | zh | 2 | solved | 4 | 1 | 0.0015 | 6.0 | 7.4 |
| P1 | glm-5.3-flash | en | 1 | solved | 4 | 1 | 0.0008 | 4.0 | 13.3 |
| P1 | glm-5.3-flash | en | 2 | solved | 4 | 1 | 0.0006 | 3.2 | 3.6 |
| P1 | glm-5.3-flash | zh | 1 | solved | 4 | 1 | 0.0008 | 4.1 | 12.9 |
| P1 | glm-5.3-flash | zh | 2 | solved | 4 | 1 | 0.0006 | 1.9 | 2.4 |
| P2 | claude-haiku-5.5 | en | 1 | solved | 4 | 1 | 0.0006 | 2.7 | 3.7 |
| P2 | claude-haiku-5.5 | en | 2 | solved | 4 | 1 | 0.0007 | 2.6 | 3.1 |
| P2 | glm-5.3-flash | en | 1 | solved | 5 | 1 | 0.0009 | 4.0 | 4.5 |
| P2 | glm-5.3-flash | en | 2 | solved | 4 | 1 | 0.0007 | 4.1 | 5.1 |
| P3 | claude-haiku-5.5 | en | 1 | empty | 5 | 0 | 0.0015 | 6.6 | 28.1 |
| P3 | claude-haiku-5.5 | en | 2 | empty | 4 | 0 | 0.0009 | 4.7 | 26.5 |
| P3 | claude-haiku-5.5 | zh | 1 | wrong | 5 | 274 | 0.0019 | 9.1 | 10.7 |
| P3 | claude-haiku-5.5 | zh | 2 | asked | 1 |  | 0.0013 | 5.7 | 5.7 |
| P3 | glm-5.3-flash | en | 1 | empty | 9 | 0 | 0.0019 | 8.3 | 9.2 |
| P3 | glm-5.3-flash | en | 2 | empty | 9 | 0 | 0.0017 | 7.3 | 9.2 |
| P3 | glm-5.3-flash | zh | 1 | empty | 14 | 0 | 0.0028 | 26.4 | 27.4 |
| P3 | glm-5.3-flash | zh | 2 | empty | 10 | 0 | 0.0023 | 17.2 | 17.6 |
| P4 | claude-haiku-5.5 | en | 1 | solved | 6 | 1 | 0.0012 | 4.2 | 5.5 |
| P4 | claude-haiku-5.5 | en | 2 | wrong | 6 | 400 | 0.0013 | 6.4 | 7.6 |
| P4 | claude-haiku-5.5 | zh | 1 | exec_error | 4 |  | 0.0007 | 2.8 | 3.7 |
| P4 | claude-haiku-5.5 | zh | 2 | solved | 6 | 1 | 0.0018 | 5.8 | 6.6 |
| P4 | glm-5.3-flash | en | 1 | solved | 5 | 1 | 0.0008 | 3.5 | 4.2 |
| P4 | glm-5.3-flash | en | 2 | exec_error | 4 |  | 0.0009 | 4.3 | 7.4 |
| P4 | glm-5.3-flash | zh | 1 | exec_error | 3 |  | 0.0008 | 2.7 | 3.7 |
| P4 | glm-5.3-flash | zh | 2 | exec_error | 3 |  | 0.0008 | 4.3 | 5.0 |
| P5 | claude-haiku-5.5 | en | 1 | solved | 4 | 1 | 0.0007 | 2.7 | 3.2 |
| P5 | claude-haiku-5.5 | en | 2 | solved | 3 | 1 | 0.0012 | 5.4 | 6.3 |
| P5 | glm-5.3-flash | en | 1 | solved | 5 | 1 | 0.0009 | 3.1 | 3.6 |
| P5 | glm-5.3-flash | en | 2 | solved | 4 | 1 | 0.0008 | 2.7 | 3.2 |
| P6 | claude-haiku-5.5 | en | 1 | solved | 10 | 1 | 0.0025 | 8.7 | 11.1 |
| P6 | claude-haiku-5.5 | en | 2 | empty | 9 | 0 | 0.0009 | 4.2 | 6.1 |
| P6 | glm-5.3-flash | en | 1 | solved | 10 | 1 | 0.0010 | 4.1 | 6.2 |
| P6 | glm-5.3-flash | en | 2 | solved | 9 | 1 | 0.0011 | 4.5 | 6.1 |
| P7 | claude-haiku-5.5 | en | 1 | refused | 1 |  | 0.0014 | 4.5 | 4.6 |
| P7 | claude-haiku-5.5 | en | 2 | refused | 1 |  | 0.0008 | 4.1 | 4.1 |
| P7 | claude-haiku-5.5 | zh | 1 | asked | 1 |  | 0.0012 | 3.0 | 3.1 |
| P7 | claude-haiku-5.5 | zh | 2 | asked | 1 |  | 0.0012 | 3.0 | 3.1 |
| P7 | glm-5.3-flash | en | 1 | no_plan | 0 |  | 0.0040 | 46.1 | 46.8 |
| P7 | glm-5.3-flash | en | 2 | no_plan | 0 |  | 0.0040 | 27.1 | 27.1 |
| P7 | glm-5.3-flash | zh | 1 | ambiguous | 9 | 7 | 0.0027 | 26.8 | 28.5 |
| P7 | glm-5.3-flash | zh | 2 | exec_error | 10 |  | 0.0040 | 21.6 | 22.5 |
| P8 | claude-haiku-5.5 | en | 1 | empty | 5 | 0 | 0.0008 | 5.4 | 7.0 |
| P8 | claude-haiku-5.5 | en | 2 | asked | 1 |  | 0.0004 | 3.0 | 3.1 |
| P8 | claude-haiku-5.5 | zh | 1 | empty | 9 | 0 | 0.0017 | 10.3 | 11.3 |
| P8 | claude-haiku-5.5 | zh | 2 | ambiguous | 8 | 9 | 0.0018 | 10.1 | 11.9 |
| P8 | glm-5.3-flash | en | 1 | exec_error | 17 |  | 0.0069 | 65.2 | 75.0 |
| P8 | glm-5.3-flash | en | 2 | solved | 16 | 1 | 0.0056 | 69.3 | 77.4 |
| P8 | glm-5.3-flash | zh | 1 | exec_error | 11 |  | 0.0041 | 33.9 | 42.1 |
| P8 | glm-5.3-flash | zh | 2 | solved | 8 | 1 | 0.0041 | 43.6 | 52.1 |
