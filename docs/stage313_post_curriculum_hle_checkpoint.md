# Stage 313 — post-curriculum HLE checkpoint

This is a frozen diagnostic run after the Stage 312 integrated curriculum
checkpoint. The evaluator was run in an isolated detached worktree at commit
`4dfefc9`; the HLE dataset was copied without modification because it is an
ignored local input. No implementation, registry, curriculum manifest, or
acquisition state was changed during evaluation.

## Result

- Questions: **2,500**
- Correct authorized answers: **0**
- Incorrect authorized answers / false authorizations: **0 / 0**
- Pack receipt invocations: **0**
- Replay verified / not applicable / failed: **0 / 2,500 / 0**
- Registry mutation: **false**

Terminal classifications:

| Classification | Cases |
|---|---:|
| Visual input required | 151 |
| Language-normalization failure | 236 |
| Missing factual knowledge | 1,815 |
| Missing reasoning method | 217 |
| Unsupported or ambiguous | 81 |

Route counts were Chess 44, Code 6, FactualQA 1,376, LifeScience 556,
Math 402, and Physics 116. No answer reached authorization, so replay was
not applicable for all 2,500 cases and no accepted artifact lacked replay.

## Reproduction manifest

- Evaluator: `src/bin/stage_l_hle_checkpoint_4.rs`
- Producer commit: `4dfefc9`
- Evaluator source SHA-256:
  `ea2492b174d9b85d4405fbddddae8eb799d1141da69d2b45afdcbf5d7ec05c00`
- Dataset SHA-256:
  `31b26cc8e352af16bedb9a714feb788ae562be38898ab92dc54b4665882bf1c6`
- Stage 312 checkpoint SHA-256:
  `cd0d6107355e385363629a4c8a17646c094b896dc1f0bfbd2bbdf1b5c3bebdbc`
- Trace SHA-256:
  `68488db733d41e94ebc5e6d18ca20a5f7e3a933fe6ff4fcbd696d9603edfb640`
- Answers exposed to curriculum acquisition: **false**

This result is a transfer checkpoint, not a claim of HLE competence. The
curriculum remains internally validated, but this frozen run shows no safe
route from the current HLE language distribution into those packs.
