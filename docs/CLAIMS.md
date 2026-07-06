# Claim Ledger

This file tracks the research claims the architecture is trying to make true.
It is not a marketing document.  A claim can be proven, supported, provisional,
false, or retired.

## Status Labels

- `proven`: algebraic or theorem-backed under stated assumptions.
- `supported`: repeatedly observed in tests or experiments.
- `provisional`: plausible, but evidence is incomplete.
- `false`: contradicted by experiment or implementation evidence.
- `retired`: no longer part of the active architecture.

## Claim Template

Use this shape for new claims:

```text
ID:
Status:
Statement:
Owner modules:
Evidence:
Baseline:
Failure condition:
Next check:
```

## Active Claims

### C-001: Bounded Bitwise Memory

Status: `supported`

Statement: The accumulator and cluster representation can preserve influence
from an unbounded stream while keeping memory bounded with respect to time.

Owner modules: `src/lib.rs`, `src/reason.rs`, `src/sleep.rs`.

Evidence: accumulator tests, cluster-count bounds, decay and compaction checks,
and the formal memory arguments in `MATH.md`.

Baseline: unbounded append-only event memory.

Failure condition: cluster count, accumulator state, or retained entries grow
without bound under a stationary or slowly drifting input process.

Next check: add a structured long-run memory-pressure benchmark with fixed
input seed and machine-readable output.

### C-002: Projection Stabilizes Noisy State

Status: `supported`

Statement: Projection onto learned centroids suppresses random bit noise while
preserving task-relevant state.

Owner modules: `src/reason.rs`, `src/lib.rs`, `src/hierarchy.rs`.

Evidence: projection tests, contraction telemetry, soft projection calibration,
and theorem sections in `MATH.md`.

Baseline: raw hypervector state without centroid projection.

Failure condition: projected outputs become less predictive or less stable than
raw inputs under the same noise process.

Next check: keep a fast deterministic projection invariant test and a separate
ignored calibration benchmark for soft projection.

### C-003: Transition Structure Can Create Higher Concepts

Status: `provisional`

Statement: L2 concepts can emerge from temporal and mutual-transition structure,
not only from direct centroid similarity.

Owner modules: `src/abstractor.rs`, `src/temporal.rs`, `src/hierarchy.rs`.

Evidence: community-deduplication tests and abstractor integration experiments.

Baseline: nearest-centroid grouping without transition information.

Failure condition: transition-aware abstraction does not improve prediction,
compression, or transfer over similarity-only grouping.

Next check: add an ablation benchmark comparing abstractor on/off over the same
seeded transition stream.

### C-004: Self-Extending Diagnostics Improve Adaptation

Status: `provisional`

Statement: The diagnostic learner can promote recurring failure patterns into
operational categories that improve later diagnosis.

Owner modules: `src/abstraction_learner.rs`, `src/diagnostic.rs`,
`src/meta_reasoning.rs`.

Evidence: diagnostic learner tests and recent persistence-threading work.

Baseline: static keyword/category maps.

Failure condition: promoted categories do not improve held-out diagnosis, or
they degrade existing categories through overgeneralization.

Next check: persist learner promotions with version metadata and measure
pre/post behavior on held-out diagnostic variants.

### C-005: VSA Analogy Transfers Structure Across Domains

Status: `false`

Statement: The current VSA analogy stack preserves enough structure for
zero-overlap structural transfer.

Owner modules: `src/analogy.rs`, `src/bin/intervention_test.rs`, `MATH.md`.

Evidence: `MATH.md` records Assumption A21 as empirically false for the current
approach, and `intervention_test` documents that the VSA architecture
contributed nothing to the zero-overlap classification result.

Baseline: non-VSA structural parser or keyword classifier.

Failure condition: already met for the current implementation.

Next check: keep this as a negative result until a new representation or parser
changes the underlying mechanism.

### C-006: Tool Use Can Be Made Auditable And Learnable

Status: `provisional`

Statement: External tool use can be represented as structured memory events so
the system can learn tool reliability and explain action choice.

Owner modules: `src/action.rs`, `src/actuator.rs`, `src/code_bridge.rs`,
`src/sensory.rs`, `src/cognition.rs`.

Evidence: current action and actuator surfaces exist.  `ToolEvent` now records
intent, request, optional result, side-effect class, confidence, and memory
updates, but tool reliability is not yet a first-class learned state.

Baseline: direct tool invocation with only log output.

Failure condition: the system cannot reconstruct why a tool was called, what it
changed, or whether similar calls succeeded in the past.

Next check: route real actuator calls through `ToolEvent` and aggregate success
rates by action type and side-effect class.

### C-007: Autonomy Requires Operator-Visible Boundaries

Status: `provisional`

Statement: Long-running autonomous behavior should be constrained by explicit
budgets, audit records, rollback points, and fail-closed behavior.

Owner modules: `src/bin/autonomy_experiment.rs`, `src/bin/validate_autonomy.rs`,
`src/monitor.rs`, `src/workspace.rs`, `src/defense.rs`, `src/cognition.rs`.

Evidence: simulated autonomy experiments and monitoring primitives exist.
`AutonomyBudget` now accounts for action count, elapsed time, external writes,
and maximum risk, but budgeted action governance is not yet central.

Baseline: unconstrained goal loop.

Failure condition: the system can take external actions without a replayable
decision record or clear budget accounting.

Next check: require real external actions to spend from `AutonomyBudget` and
emit a replayable `ToolEvent`.

### C-008: QA Term Resolution Can Be Audited

Status: `supported`

Statement: Term resolution can expose the mechanism that produced each query
vector without changing legacy QA behavior.

Owner modules: `src/qa.rs`, `src/cognition.rs`, `MATH.md`.

Evidence: `resolve_term_trace` returns `ResolveTrace`, and `resolve_term` delegates
to `resolve_term_trace(...).vector`. Resolver tests verify exact cluster, raw
fallback, and association traversal provenance.  QA episode wrappers now attach
term traces and confidence to combined-answer and chain-answer outputs without
changing legacy answer strings.

Baseline: `resolve_term` returned only a hypervector, so failures could not be
assigned to raw encoding, cluster projection, or association traversal.

Failure condition: a trace reports a source, centroid, label, association, or
confidence that does not match the branch used to construct the returned vector.

Next check: persist `CognitiveEpisode` records for QA runs and connect outcomes
to reversible memory updates.

### C-009: Cognitive Episodes Can Carry Feedback

Status: `provisional`

Statement: A reasoning attempt can be represented as a replayable episode with
input, answer, confidence, trace evidence, outcome, memory updates, and active
ablation flags.

Owner modules: `src/cognition.rs`, `src/qa.rs`.

Evidence: `CognitiveEpisode`, `EpisodeOutcome`, `MemoryUpdate`, and
`AblationConfig` exist as serializable data structures.  QA wrappers create
episodes for combined and chain answers, and unit tests cover episode creation
and unknown-answer confidence handling.

Baseline: answer strings and printed logs with no structured outcome or
feedback carrier.

Failure condition: the system cannot replay what evidence supported an answer,
what feedback was applied, or which architecture components were enabled.

Next check: add a small feedback store that appends episode outcomes and applies
only reversible memory updates until evaluation proves the update rule.

### C-010: Experiments Can Be Compared By Structured Result Records

Status: `provisional`

Statement: Broad research experiments can stay comparable if each run records
claim, commit, seed, dataset, baseline, metrics, pass/fail, and notes.

Owner modules: `src/cognition.rs`, `docs/EVALUATION.md`.

Evidence: `ExperimentResult` mirrors the evaluation matrix result schema and
has unit coverage for metric lookup.

Baseline: interpreting free-form test output manually.

Failure condition: two runs of the same experiment cannot be compared without
reading logs by hand.

Next check: make at least one ignored benchmark emit `ExperimentResult` JSON.

## Retired Or Negative Claims

Negative results are useful research output.  Do not delete them just because
they are inconvenient.

- Assumption A21, abstraction preservation: current evidence says the existing
  abstraction map does not preserve task-relevant causal structure for held-out
  structural variants.
- Earlier soft-projection formula claims before the v3.1 correction: superseded
  by the corrected formula and calibration.
- Earlier `L_F <= 0.5` bound: superseded by the tighter `L_F <= 1.0` correction.
