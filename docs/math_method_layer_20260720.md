# Typed mathematical-method layer (2026-07-20)

The five reconnaissance scans found no non-empty bounded calculator island in
the sampled HLE corpus.  The next layer is therefore an authorization
substrate for specialized methods, not another executor.

`src/math_methods.rs` provides:

- `MathematicalFact` and `GroundedFact`, with explicit/normalized/derived
  status and source provenance;
- `TaskShape`, `MathDomain`, and `FactKind` for structural retrieval;
- `MathematicalMethodSpec` with premise/conclusion patterns, assumptions, side
  conditions, capabilities, verification strategy, and audited provenance;
- schema validation for provenance, binding scope, conclusion shape, produced
  fact kinds, and duplicate method IDs;
- `MathematicalMethodRegistry::retrieve`, which only performs explainable
  domain/task/fact-shape filtering;
- strict `instantiate_with_context`, which requires authoritative grounded
  facts, consistent bindings, explicit assumptions, and discharged side
  conditions before producing typed facts.
- `MathematicalDerivationStep`/`MathematicalDerivationPlan` receipts that retain
  premise IDs, derived-fact provenance, and unresolved obligations.
- `plan_one_step`, with hard candidate limits and explicit `Unique`,
  `Consensus`, `Ambiguous`, and `None` outcomes; registry order cannot resolve
  conflicting methods.

Retrieval does not authorize execution.  The module deliberately has no CAS
call and no semantic-similarity fallback.  A future method pack must be chosen
from a recurring HLE cluster and validated independently before it is wired to
an executor.

The initial unit tests use a definition-application method as a schema fixture;
it is not enabled as a benchmark answer path.
