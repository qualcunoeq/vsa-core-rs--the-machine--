# Phase 31 — Shadow law lookup and equation binding

Phase 31 adds two generic, non-authorizing bridges for the retrieval-ready
scientific-law audit:

```text
normalized law name + domain
  → provenance-bearing LawRecord candidate(s)

LawRecord + typed quantities
  → unit-checked BoundEquationArtifact
  → existing symbolic capability (future integration)
```

The bridges contain no law catalog, retrieval backend, router mutation, or
answer authorization. Callers supply `LawRecord` values, keeping law content
and provenance separate from bridge behavior.

## Contract boundaries

`named_law_lookup` returns `Unique` only when the normalized name/alias and
optional domain identify exactly one record and all requested variables are in
that record. Missing names, unknown laws, overloaded aliases, and variables
outside the record remain non-authorizing (`Missing`, `Unsupported`, or
`Ambiguous`).

`equation_binding` returns a typed artifact only when the requested output is a
law variable, every other law variable is bound exactly once, bindings use
known symbols, and supplied units agree with the law's explicit unit
constraints. Missing bindings, duplicate symbols, incompatible units, and
unknown symbols are rejected or held ambiguous. The artifact preserves law,
quantity, assumption, validity-domain, and provenance information.

Every result has a deterministic replay hash. Replay recomputes the hash over
the complete result payload; tampering therefore fails closed.

## Independent boundary corpus

The benchmark uses caller-supplied fixture records for Ohm's law, Newton's
second law, the ideal-gas law, and an intentionally overloaded `energy law`
alias. It contains positive aliases, ambiguous aliases, missing/unsupported
names, complete bindings, missing/unitless bindings, duplicate bindings,
unknown outputs, and an incompatible-unit case. The fixtures are benchmark
scaffolding, not external scientific evidence.

Run:

```text
cargo run --quiet --bin hle_law_bridge_bench -- docs/phase31_hle_law_bridge_bench.json
```

The immutable machine-readable report is
[`phase31_hle_law_bridge_bench.json`](phase31_hle_law_bridge_bench.json),
SHA-256:

```text
32279ef9b586d0ae7518950fa6e0270aa7c9d656784db9f7f3c4478469966bf4
```

Results:

| Metric | Result |
| --- | ---: |
| Corpus cases | 23 |
| Lookup cases | 14 |
| Binding cases | 9 |
| Unique lookups | 6 |
| Ambiguous lookups | 4 |
| Unsupported/missing lookups | 4 |
| Complete bindings | 4 |
| Rejected bindings | 5 |
| Replay-verified results | 23/23 |
| False authorizations | 0 |
| Retrieval-ready HLE cases observed | 12 |
| HLE answers authorized by this shadow benchmark | 0 |

The HLE count is diagnostic only. No law records were sourced or matched to
the 12 frozen HLE holdouts in this phase, so the bridge does not claim an HLE
score increase. Production routing and registries remain unchanged.

The report records the Phase 30 audit hash (`9fbe52a26b378c16e858bca75ca2835b5339aae5c31602e068b446205956c0ed`), the corpus hash, and
`registry_mutated: false` so the evaluation target and authorization boundary
are reproducible.

## Focused verification

```text
cargo test --lib law_bridge --quiet
```

The focused module suite passes all three tests. The repository continues to
contain unrelated pre-existing warnings in the broader library build.

## Non-goals

This phase does not retrieve facts, choose among conflicting external sources,
solve equations, alter the HLE release, or promote a law capability. Those are
separate follow-up experiments requiring independently sourced law records and
an untouched HLE holdout.
