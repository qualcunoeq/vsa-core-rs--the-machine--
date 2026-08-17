# Stage 106 — Frozen HLE checkpoint after set/counting curriculum growth

This checkpoint reruns the unchanged 2,500-question HLE dataset with the current curriculum manifest. The signal audit now includes finite-set and counting terminology; no pack implementation or production routing was changed.

| Metric | Result |
|---|---:|
| HLE cases | 2,500 |
| Correct authorized | 2 |
| Incorrect authorizations / false authorizations | 0 / 0 |
| Curriculum signals | 597 |
| Pack invocations | 0 |
| Compatibility replay | 2/2 |
| Replay not recorded | 0 |
| Visual-required | 260 |
| No curriculum signal | 1,695 |
| Unresolved | 543 |
| Registry mutation | false |

The dataset hash remains `31b26cc8e352af16bedb9a714feb788e562be38898ab92dc54b4665882bf1c6`. The current manifest hash and complete terminal summary are recorded in `stage106_hle_curriculum_checkpoint.json`.
