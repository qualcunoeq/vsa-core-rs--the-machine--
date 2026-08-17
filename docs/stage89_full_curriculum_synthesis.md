# Stage 89 — full current-curriculum synthesis

Stage 89 is a larger integration gate after the individual curriculum and
source-education campaigns.  The independently authored corpus contains 2,000
route-blind cases across eight validated families:

* abstract algebra → number theory;
* combinatorial counts → finite probability;
* finite graph → adjacency linear algebra;
* spectral linear algebra → polynomial evaluation → number theory;
* bounded ODE → exact calculus;
* finite probability → one-step Markov evolution;
* source-derived finite sequence language → source formula catalog;
* source-derived unit language → source conversion catalog.

Each family contains 150 supported, 50 ambiguous, and 50 refused cases.  A
supported route is authorized only after every typed handoff is complete and
all emitted receipts replay.  Ambiguous and refused routes are retained as
diagnostic failures rather than being forced into a downstream pack.

| Measure | Result |
|---|---:|
| Cases | 2,000 |
| Supported / ambiguous / refused | 1,200 / 400 / 400 |
| Exact route decisions | 2,000/2,000 |
| Supported authorizations | 1,200/1,200 |
| Semantic handoffs | 1,200/1,200 |
| Emitted intermediate artifacts | 3,400 |
| Case replay verification | 2,000/2,000 |
| Tamper rejection | 2,000/2,000 |
| False authorizations / denials | 0 / 0 |

The refusal gates remain explicit: unresolved coprimality, nonunits,
ambiguous vertex ordering, invalid graph domains, unresolved stochastic
conventions, over-budget Markov evolution, continuous/discrete semantic
ambiguity, unsupported ODE operations, spectral-domain boundaries, and
unsupported or ambiguous source-language targets.  The source catalog hashes
and complete per-case receipts are recorded in
`stage89_full_curriculum_synthesis.json`.

Reproduction:

```text
RUSTFLAGS='-Awarnings' cargo run --quiet --bin stage89_full_curriculum_synthesis
```

This remains shadow-only.  It does not read HLE, mutate the live router or
registry, or alter the curriculum manifest.
