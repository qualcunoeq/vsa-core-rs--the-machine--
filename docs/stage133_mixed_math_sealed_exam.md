# Stage 133 — mixed mathematical sealed technical-language exam

Stage 133 evaluates the three new route-blind frontends on an independently
authored shifted corpus partitioned before execution:

* finite simplicial homology;
* bounded elementary number theory;
* bounded finite Dirichlet characters.

The 1,200 reports are evenly partitioned into development, validation, and
sealed holdout sets. Each partition contains 240 supported, 80 ambiguous, and
80 unsupported reports. The sealed partition is not used to select grammar or
route behavior; it is evaluated only after the corpus and implementation are
fixed.

| Measure | Development | Validation | Sealed |
|---|---:|---:|---:|
| Cases | 400 | 400 | 400 |
| Supported / ambiguous / unsupported | 240 / 80 / 80 | 240 / 80 / 80 | 240 / 80 / 80 |
| Exact route decisions | 400/400 | 400/400 | 400/400 |
| Supported authorizations | 240/240 | 240/240 | 240/240 |
| Frontend replay verified | 400/400 | 400/400 | 400/400 |
| Downstream artifacts replayed | 240/240 | 240/240 | 240/240 |
| Frontend tamper rejected | 400/400 | 400/400 | 400/400 |
| Downstream tamper rejected | 240/240 | 240/240 | 240/240 |
| False authorizations / denials | 0 / 0 | 0 / 0 | 0 / 0 |

Hashes:

* full corpus: `b5c5e98b751ae94c73b6a8abf37eb58918d43536b2b41a9764a3330b7279b7f2`
* development: `dd89729ccebda4bffed4d90cef047ace48f9f6b7b366e296863c58fa9268eabc`
* validation: `461d1722cfebb4d3acea3d1072ff733b1af271bec32aa91c15f30bc5463e9701`
* sealed: `66608af83da9100d4432d1e9989f1dd470c62aaa9231dba4c7c2521cc7621f7f`

Reproduce with:

```text
cargo run --quiet --bin stage133_mixed_math_sealed_exam
```

The machine-readable report is
`docs/stage133_mixed_math_sealed_exam.json`. This remains a shadow checkpoint;
the production registry, curriculum manifest, and HLE holdout are unchanged.
