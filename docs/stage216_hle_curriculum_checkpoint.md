# Stage 216 — HLE checkpoint with direct-answer replay repair

The evaluator now replays direct authorized answers by rerunning the same
deterministic orchestration and comparing answer, evidence, and verification.
Plan-backed answers retain their existing receipt gate.

| Measure | Result |
|---|---:|
| Questions | 2,500 |
| Correct authorized answers | 2 |
| Incorrect authorized answers | 0 |
| False authorizations | 0 |
| Replay verified | 2 |
| Replay not applicable | 2,498 |
| Replay not recorded | 0 |
| Replay mismatches | 0 |
| Curriculum signals / pack invocations | 705 / 0 |
| Registry / ontology mutation | 0 / 0 |

The first-failure distribution is unchanged from Stage 214: 260 visual,
1,614 no-signal, 65 language-normalization, 13 unsupported-target, 451
missing-factual-prerequisite, 87 missing-specialist-theorem, and 8
unestablished-assumption cases, plus the 2 authorized matches.

Dataset SHA-256: `31b26cc8e352af16bedb9a714feb788ae562be38898ab92dc54b4665882bf1c6`  
Manifest SHA-256: `5be43e121500a591b8b380a029a155c8cdafa657b97bbf4756176d39c6560bc8`  
Summary SHA-256: `e21387a4ced5307acc1f490f50d583969fe206580613c6c10dd3efea4d0efbda`  
Trace SHA-256: `c3da954c1c551c5e427f296ea9f07c56b48eb7b339c44f96f32a5aede74fd53e`  
