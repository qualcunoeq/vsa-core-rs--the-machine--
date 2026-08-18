# Stage 212 — frozen HLE checkpoint after Möbius/frontend growth

This is a fresh frozen evaluation at producer commit `a73b1ff`. The HLE
dataset was not used for development or routing changes during the run.

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
Summary SHA-256: `83c4d3e4d930a12682ca3c6be42b1e4360b50867dae3cf8fd6c9b64e413ff867`  
Trace SHA-256: `043b210c4280061c8d22a94148597c8c0327f54ca372bac28071a909318a848d`  

The complete machine-readable summary and 2,500-line route trace are kept in
the adjacent JSON and `.trace.jsonl` artifacts. This checkpoint demonstrates
that the additional Möbius capability has not yet transferred to HLE; it does
not claim a score increase or any production authorization.
