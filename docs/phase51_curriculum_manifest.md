# Phase 51 — Governed breadth-first curriculum

Phase 50 established that the clean HLE seeds are unrelated specialist
singletons. Phase 51 records the alternative strategy: build reusable domain
packs from external sources, in prerequisite order, while keeping HLE strictly
as a frozen evaluation holdout.

The manifest is planning-only. It does not add a pack to the registry, alter
routing, or authorize answers.

## Planned sequence

```text
linear algebra / spectral theory
├── probability / stochastic processes
├── real / complex analysis
├── graph theory / spectral inequalities
└── abstract algebra
    └── topology / geometric invariants

real / complex analysis + abstract algebra
└── number theory
```

The existing classical-mechanics pack is retained as a shadow-validated
substrate, not as a claim about HLE overlap.

## Every pack must pass

Before promotion, a pack must have:

* authoritative, provenance-bearing sources;
* an independently authored development corpus;
* explicit supported, ambiguous, and unsupported boundaries;
* an adversarial pressure corpus;
* replay-verified artifacts and executions;
* zero false authorization;
* a frozen HLE holdout that is never used for development.

The manifest currently contains eight packs: one shadow-validated mechanics
substrate and seven planned foundational domains. No planned pack is promoted,
and the emitted manifest reports zero production authorizations.

Run the deterministic manifest emitter with:

```text
cargo run --quiet --bin curriculum_manifest
```
