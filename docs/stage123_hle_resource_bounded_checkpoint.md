# Stage 123 — Resource-bounded HLE checkpoint

The HLE evaluator now supports an explicit formula-cache bound for constrained
runs. Default runtime behavior remains unlimited; this checkpoint records a
256-entry bound and keeps the HLE dataset and router unchanged.

| Metric | Result |
|---|---:|
| Cache limit | 256 formulas |
| HLE cases | 2,500 |
| Dataset hash | `31b26cc8e352af16bedb9a714feb788e562be38898ab92dc54b4665882bf1c6` |
| Correct authorized | 2 |
| Incorrect authorized / false authorization | 0 / 0 |
| Curriculum signals | 606 |
| Pack invocations | 0 |
| Replay compatibility | 2/2 |
| Replay not applicable | 2,498 |
| Replay not recorded | 0 |
| Visual required | 260 |
| Registry mutation | 0 |

The result matches the previous HLE baseline while making resource usage
explicit. It is not directly comparable to an unlimited-cache run without the
recorded cache configuration.
