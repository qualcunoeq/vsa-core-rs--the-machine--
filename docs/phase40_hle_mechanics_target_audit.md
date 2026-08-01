# Phase 40 — HLE mechanics target-shape audit

Phase 40 audits the 152 HLE questions that had a mechanics vocabulary signal
but failed Phase 39 at `target_not_groundable`. It is a diagnostic taxonomy
pass only: no law is selected, no capability is invoked, and no production
route changes.

## Reproducibility

```text
cargo test --lib mechanics_situation --quiet
cargo run --quiet --bin hle_mechanics_target_audit -- \
  docs/phase40_hle_mechanics_target_audit.json
```

The audit is keyed to the immutable Phase 39 report and the frozen HLE input.
The report stores question hashes, candidate target families, candidate
physics subdomains, and lexical indicators. Multiple candidates are preserved
as `ambiguous`; no lexical label is treated as an authorization.

| Artifact | SHA-256 |
| --- | --- |
| HLE input | `31b26cc8e352af16bedb9a714feb788ae562be38898ab92dc54b4665882bf1c6` |
| Phase 39 report | `f53668c01485846c41af15f8087bbcf256abb2ee57de30889631bbb44a8c8adf` |
| Phase 40 report | `95541bb22ae43dd14e14b8ec715ed256d6208e0346ce7964a250b0102aef88b9` |

## Target artifact families

| Family | Cases |
| --- | ---: |
| unclassified | 82 |
| ambiguous candidates | 18 |
| physical phenomenon/model | 12 |
| specialist factual target | 12 |
| named object/theorem/convention | 8 |
| asymptotic/scaling law | 5 |
| integral/differential equation | 4 |
| spectrum/eigenvalue/mode | 4 |
| optimization/bound | 4 |
| conceptual consequence | 2 |
| dimensionless parameter | 1 |
| prove/derive relation | 1 |

The high `unclassified` count is itself evidence: many “mechanics” signals
come from incidental quantities or embedded material in multidisciplinary
questions, while the requested output is not exposed by a safe local marker.
The 18 ambiguous records retain all candidate families rather than collapsing
to a guessed target.

## Physics subdomains

| Subdomain | Cases |
| --- | ---: |
| unclassified | 126 |
| continuum mechanics | 6 |
| ambiguous candidates | 5 |
| relativity | 5 |
| dynamical systems | 4 |
| mathematical physics | 3 |
| statistical mechanics | 2 |
| field theory | 1 |

The subdomain distribution confirms that elementary translational mechanics
is not a meaningful description of this HLE tail. Most records require either
specialist context not represented by the current ontology or a target shape
that cannot be inferred safely from vocabulary alone.

## Interpretation

The target audit does not justify a new broad mechanics capability. The useful
next selection criterion is typed target overlap: choose a domain only when
multiple questions share an output artifact, prerequisites, and invariants,
and when an independent external corpus can establish that contract.

This phase preserves the zero-authorization boundary and leaves HLE and
production routing unchanged.
