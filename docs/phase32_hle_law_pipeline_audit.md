# Phase 32 — HLE law-pipeline obstruction audit

Phase 31 proved the generic lookup and binding bridges, but none of the 12
HLE cases reached an executable answer route. Phase 32 audits those cases
without adding law content or changing authorization. Every record preserves
the original question, source hashes, bridge selection, candidate lookup,
binding/equation status, downstream route, and the first failing gate.

## Result

| First failing gate | Cases |
| --- | ---: |
| `no_uniquely_matched_law` | 8 |
| `unsupported_equation_shape` | 4 |
| Total | 12 |

All 12 audit records replayed deterministically. No answer was authorized and
the registry remained unchanged.

The eight named-law cases do not contain a uniquely matchable record in the
Phase 31 fixture catalog. Several Phase 30 cues are generic (`law`) or are
domain descriptors (`treatment effect`, `thermodynamic`) rather than canonical
law identifiers. The audit uses token boundaries, so a substring such as
`law` inside another word cannot create a candidate.

The four equation-binding cases were selected by Phase 30 from heuristic math
spans, but no typed law equation was extracted for the requested target. Their
first obstruction is therefore the equation shape/target handoff, before any
symbol binding or downstream solver call. This explains why the four complete
bindings in the independent Phase 31 corpus did not translate into HLE
answers: those bindings were supplied typed artifacts, while the HLE cases
still lack the language-to-artifact step.

## Audit artifact

The machine-readable case-level report is
[`phase32_hle_law_pipeline_audit.json`](phase32_hle_law_pipeline_audit.json),
SHA-256:

```text
11e1598ee3a8a832291a301a34863dc6fbabd18121dee7eacc23484c8f174600
```

Inputs recorded by the report:

* Phase 30 audit SHA-256: `9fbe52a26b378c16e858bca75ca2835b5339aae5c31602e068b446205956c0ed`;
* regenerated HLE trace SHA-256: `696ad4e5cc2c3475d5a5c3b5cd4ca691e0317d64adf17b075b9c01f4d2a596c9`;
* Phase 31 bridge report SHA-256: `32279ef9b586d0ae7518950fa6e0270aa7c9d656784db9f7f3c4478469966bf4`.

Run:

```text
cargo run --quiet --bin hle_law_pipeline_audit -- \
  docs/phase30_hle_law_audit.json \
  /tmp/hle_phase26_combined.traces.jsonl \
  docs/phase32_hle_law_pipeline_audit.json
```

The independent bridge reference retained in the report is 23 corpus cases,
4 complete bindings, 5 rejected bindings, 23 replay checks, and 0 false
authorizations.

## Consequence

The next implementation target is not a larger law registry. It is the missing
typed handoff:

```text
HLE question
→ identify the operative law/equation region
→ construct a provenance-bearing LawRecord or typed equation
→ bind question quantities and assumptions
→ invoke an existing solver
→ verify answer format and replay
```

The audit deliberately stops before retrieval and solving. Phase 32 therefore
claims no HLE score increase; production routing and authorization remain
unchanged.
