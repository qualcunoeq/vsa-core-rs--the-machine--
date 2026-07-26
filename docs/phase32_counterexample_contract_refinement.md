# Phase 3B — Counterexample-Driven Contract Refinement

Phase 3B evaluates a proposed capability contract against the independently
verified `CandidateCorpus` produced by Phase 3A. It is diagnostic only:
counterexamples can produce a revision proposal, but they never mutate a
proposal, registry, router, or executor.

## Pipeline

```text
CandidateCorpus
    ↓
independent oracle verification
    ↓
proposal/oracle comparison
    ↓
typed counterexamples
    ↓
spec-first minimization
    ↓
bounded revision proposal
```

`evaluate_candidate_corpus` compares each oracle-verified case with the
proposal's own synthesized decision. Exact prompt matches are preferred;
semantically similar prompts are considered only above the deterministic
similarity threshold. Missing matches are reported as missing contract
coverage rather than being treated as proposal successes.

## Failure taxonomy

`ContractFailureKind` distinguishes false applicability, false ambiguity,
false unsupported decisions, wrong form selection, missing forms, overly
strict requirements, missing safety predicates, incorrect ambiguity causes,
and bridge mismatches.

Every `ContractCounterexample` retains its typed `CaseSpec`, rendered prompt,
oracle decision and reasoning, actual proposal decision, matched form, family,
and evidence provenance.

## Bounded repairs

`minimize_counterexample` removes typed bindings one at a time and keeps a
removal only when the same failure class remains. It never performs arbitrary
string deletion.

`propose_contract_revision` emits a `ContractRevisionProposal` containing only
diagnostic edits. A new supported form requires at least two counterexamples in
one coherent family; a single severe false acceptance may justify a predicate
tightening proposal. Revision complexity is reported explicitly.

`apply_revision_sandboxed` applies those edits only to a cloned
`ProposalPipelineResult`; `evaluate_revision_sandboxed` computes before/after
boundary deltas without changing the parent. `RevisionHistory` records
canonical revision fingerprints and enforces a caller-selected iteration
budget. Repeated fingerprints are classified as oscillation.

The defect-injection helpers cover removal of safety predicates, spurious
requirements, missing forms, collapsed ambiguity, wrong bridges, and broadened
numeric forms. They are test fixtures, not production proposal mutations.

The regression campaign exercises these controls against the four historical
families (`QuantityRelationV1`, `UnitQuantity`, `FractionalQuantity`, and
`PercentageQuantityV1`) and requires each observable injected defect to produce
an independently classified counterexample.

## Reproducible evidence

The focused proposer suite currently reports **52 passed, 0 failed**. The
Phase 3B fixtures cover **4 historical capability families** and the complete
six-class defect vocabulary:

| Evidence item | Recorded result |
| --- | ---: |
| Focused proposer tests | 52 / 52 passed |
| Historical capability families | 4 |
| Injected defect classes | 6 |
| Generic campaign defect classes observed | 6 / 6 |
| Historical-family minimum coverage | ≥2 observed classes per family |
| Live proposal/registry mutation | 0 |
| Revision iteration budget | caller-selected; campaign uses 3 |

Each observed defect produces at least one oracle-verified counterexample and
an expected repair category. The campaign also records the fields needed for
counterexample discovery, minimized witnesses, revision fingerprints, and
before/after boundary deltas. Repair-category accuracy, accepted/rejected
revision counts, and aggregate recall deltas remain diagnostic outputs of the
sandbox evaluation rather than promotion claims; they are intentionally not
reported as measured successes until a revision-application campaign exercises
them across independent corpora.

## Safety boundary

Phase 3B does not execute a proposed revision and does not grant capability
authority. Revision history, global regression checks, oscillation detection,
and final approval remain caller/governance responsibilities.

The injected-defect regression test demonstrates the intended loop:

```text
weakened contract
→ independent counterexample
→ typed failure classification
→ minimized witness
→ bounded revision proposal
```
