# Stage 445 — development-only external portfolio score

This is a hash-only evaluation of the route-blind Goal 6 portfolio on the
development partition. It reads development answer hashes only; no sealed
questions or sealed answers are read, and no production route or curriculum
manifest is mutated.

## Result

| Metric | Result |
|---|---:|
| Questions / development answer hashes | 3000 / 3000 |
| Unique executable candidates | 24 |
| Correct candidate hashes | 24 |
| Incorrect candidate hashes | 0 |
| Candidate replay verification | 24 / 24 |
| Multiple-route ambiguities / no executable route | 0 / 2976 |
| Plaintext answers / sealed questions read | 0 / 0 |
| Production authorizations / false authorizations | 0 / 0 |
| Registry or curriculum mutation | 0 |

The 24 correct candidates came through ten already validated routes:

| Route | Correct candidates |
|---|---:|
| `finite_list_mean` | 10 |
| `arithmetic_sequence` | 3 |
| `base_conversion` | 3 |
| `natural_combination` | 2 |
| `arithmetic_progression_mean` | 1 |
| `bounded_counting` | 1 |
| `finite_die_experiment` | 1 |
| `mean_update` | 1 |
| `parameter_linear_system` | 1 |
| `unit_conversion` | 1 |

This is genuine development-partition transfer, but not a claim about sealed
generalization. The route portfolio remains shadow-only; the existing HLE
checkpoint remains unchanged.

## Evidence

* dataset SHA-256: `3cf924116a0f8f6a0c84d0ce7949b0c1e16221e0d4b5fcb0c4322110e30714f2`
* probe report SHA-256: `9ee2d5b5804d6126e4e0ffb68e1080aa54c05299982fe42be3857307ae3fa5a5`
* score report SHA-256: `a26b763b316374e97fa0e8f3e30e4dd19693a389aa56c4dd5b575343cf3a6706`

Reproduce with the development-only probe followed by the hash-only scorer:

```text
GOAL6_PORTFOLIO_PARTITION=development cargo run --quiet --bin goal6_external_portfolio_probe
GOAL6_PORTFOLIO_PARTITION=development GOAL6_PORTFOLIO_PRIVILEGED_EVAL=false \\
  cargo run --quiet --bin goal6_external_portfolio_score
```
