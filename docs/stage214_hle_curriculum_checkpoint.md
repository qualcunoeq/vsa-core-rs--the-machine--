# Stage 214 — current frozen HLE checkpoint after production Möbius routing

This rerun follows the production route-dispatcher integration at commit
`5b8d56e`. The dataset remained frozen and no live registry or curriculum
mutation was permitted.

| Measure | Result |
|---|---:|
| Questions | 2,500 |
| Correct authorized answers | 2 |
| Incorrect authorized answers | 0 |
| False authorizations | 0 |
| Curriculum signals | 705 |
| Pack invocations | 0 |
| Replay-compatible receipts | 0/2,500 |
| Replay not applicable | 2,498 |
| Replay not recorded | 2 historical authorized answers |
| Registry / ontology mutation | 0 / 0 |

First-failure distribution:

| Gate | Cases |
|---|---:|
| Authorized reference match | 2 |
| Visual dependency | 260 |
| No curriculum signal | 1,614 |
| Language normalization | 65 |
| Unsupported target type | 13 |
| Missing factual prerequisite | 451 |
| Missing specialist theorem | 87 |
| Assumptions not established | 8 |

Dataset SHA-256: `31b26cc8e352af16bedb9a714feb788ae562be38898ab92dc54b4665882bf1c6`  
Curriculum manifest SHA-256: `5be43e121500a591b8b380a029a155c8cdafa657b97bbf4756176d39c6560bc8`  
Summary SHA-256: `6eb06682c71096bd62bc5a3635f5a34ede3158e3443af29a93f0d4cd5361e76c`  
Trace SHA-256: `f98c7ac6f5d20601bd7a9354aea56e0b025defe7c7fa706e4815b7b494174551`  

The score remains unchanged. This is evidence of safe non-regression, not a
claim that the current foundational curriculum transfers to HLE.
