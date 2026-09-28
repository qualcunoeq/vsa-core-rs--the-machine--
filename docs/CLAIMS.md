# Claim Ledger

**Version:** v3.4
**Last updated:** 2026-09-28

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

Next check: run `test_abstraction_ablation_benchmark` across multiple seeds and
verify concept formation is consistent; measure prediction error delta between
on/off configurations across a broader range of community structures.

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

Next check: measure pre/post behavior on held-out diagnostic variants using
`TaskFamily` / `PrePostComparison` from `cognition.rs`.  Verify that version
metadata survives save/load round-trip across schema changes.

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

Evidence: `ToolEvent` records intent, request, result, side-effect class,
confidence, and memory updates.  `ToolEventStore` provides persistent audit
log with JSON save/load.  `ToolReliabilityTracker` on `VSABrain.tool_reliability`
aggregates per-action-type EWMA success rates.  `record_tool_event()` is wired
into `run_attack_loop` after each action execution.  `SimulationMode` on
`ActionRequest` provides type-level distinction between real and simulated
actions.  6 unit tests cover store push/query/persistence, reliability tracker
EWMA/case-insensitivity, and simulation mode defaults.

Baseline: direct tool invocation with only log output; no simulated/real
distinction; no reliability aggregation.

Failure condition: the system cannot reconstruct why a tool was called, what it
changed, or whether similar calls succeeded in the past.  A simulated action
produces an external side effect (type-level confusion).

Next check: wire `record_tool_event()` into the remaining execution paths
(meta_reasoning, main.rs legacy path).  Add an integration test that exercises
the full record→store→reliability path.

### C-007: Autonomy Requires Operator-Visible Boundaries

Status: `provisional`

Statement: Long-running autonomous behavior should be constrained by explicit
budgets, audit records, rollback points, and fail-closed behavior.

Owner modules: `src/bin/autonomy_experiment.rs`, `src/bin/validate_autonomy.rs`,
`src/monitor.rs`, `src/workspace.rs`, `src/defense.rs`, `src/cognition.rs`.

Evidence: `AutonomyBudget` on `VSABrain.autonomy_budget` tracks action count,
elapsed time, external writes, and max risk.  `budgeted_execute()` in
`actuator.rs` is the central enforcement point: checks `can_spend()` before
every action, calls `spend()` after, returns `None` if budget exhausted.
`DecisionRecord` captures the full decision context (tick, intent, action,
result, pre/post budget snapshots, reasoning, tool_event_id).
`DecisionJournal` on `VSABrain.decision_journal` provides persistent audit.
4 unit tests cover enforcement flow (action cap, risk gate, external write cap,
time budget), record creation, journal query, and persistence round-trip.

Baseline: unconstrained goal loop; no budget enforcement; no decision records.

Failure condition: the system executes an external action without passing
through `budgeted_execute()` budget check, or budget exhaustion does not
produce a denial record in the decision journal.

Next check: wire `budgeted_execute()` into `solve_autonomously()` in
`meta_reasoning.rs` and the main.rs agent loop.  Persist `decision_journal`
periodically (every 50 ticks).

### C-008: QA Term Resolution Can Be Audited

Status: `supported`

Statement: Term resolution can expose the mechanism that produced each query
vector without changing legacy QA behavior.

Owner modules: `src/qa.rs`, `src/cognition.rs`, `MATH.md`.

Evidence: `resolve_term_trace` returns `ResolveTrace`, and `resolve_term` delegates
to `resolve_term_trace(...).vector`. Resolver tests verify exact cluster, raw
fallback, and association traversal provenance.  QA episode wrappers now attach
term traces and confidence to combined-answer, chain-answer, single-answer, and
verify-fact outputs without changing legacy answer strings.

Baseline: `resolve_term` returned only a hypervector, so failures could not be
assigned to raw encoding, cluster projection, or association traversal.

Failure condition: a trace reports a source, centroid, label, association, or
confidence that does not match the branch used to construct the returned vector.

Next check: add rule-level provenance to chain answers (which rules fired, in
what order, with what confidence).

### C-009: Cognitive Episodes Can Carry Feedback

Status: `provisional`

Statement: A reasoning attempt can be represented as a replayable episode with
input, answer, confidence, trace evidence, outcome, memory updates, and active
ablation flags.

Owner modules: `src/cognition.rs`, `src/qa.rs`.

Evidence: `CognitiveEpisode`, `EpisodeOutcome`, `MemoryUpdate`, and
`AblationConfig` exist as serializable data structures.  QA wrappers create
episodes for combined, chain, single-answer, and verify-fact paths.  Socket
ASK and CHAIN commands persist episodes automatically.  The episode store
is auto-saved every 50 ticks in the main agent loop.  Unit tests cover episode
creation, persistence round-trip, and unknown-answer confidence handling.

Baseline: answer strings and printed logs with no structured outcome or
feedback carrier.

Failure condition: a question asked via socket, cognition_bench, or test leaves
no episode record in the persisted store after the answer is generated.

Next check: wire episode outcomes from feedback into the store and measure
confidence calibration against observed accuracy.

### C-011: Concept Lifecycle Events Are Auditable

Status: `provisional`

Statement: All concept lifecycle events (creation, reinforcement, dissolution,
merge, decay) can be recorded in a structured, queryable, persistent journal
without changing the behavior of the abstraction or consolidation pipeline.

Owner modules: `src/cognition.rs`, `src/abstractor.rs`, `src/sleep.rs`,
`src/lib.rs`.

Evidence: `ConceptEventType` covers 7 event variants. `ConceptJournal` supports
push, query by tick/level/type, and JSON persistence. Events are wired into
`Abstractor::cycle` (creation, reinforcement, dissolution, decay crossing 0.5),
`VSABrain::compact_clusters` (merge), and `SleepCycle::cycle` (L3 creation, L2
pruning).

Baseline: concept lifecycle was observable only via ad-hoc `eprintln!` output.

Failure condition: a lifecycle event occurs (concept created, dissolved, merged,
decayed, frozen) but is not recorded in the journal when a journal is passed to
the cycle method.

Next check: wire journal into `freeze_cold_clusters`; add a benchmark that
exercises all 7 event types and verifies journal completeness.

### C-012: Concept Quality Can Be Measured Without Manual Inspection

Status: `provisional`

Statement: A concept's quality can be scored automatically from coherence,
component structure, reinforcement recency, and internal similarity, without
requiring a human to inspect centroid output.

Owner modules: `src/cognition.rs` (`ConceptQualityScore`).

Evidence: `ConceptQualityScore` composite formula weights coherence (50%),
component count (20%), freshness (20%), and internal similarity (10%). Tests
verify that high-quality concepts score > 0.70 and low-quality concepts score
< 0.50, with correct ranking and bounded results.

Baseline: concept quality was assessable only by reading cluster centroids.

Failure condition: a concept with high coherence, many components, recent
reinforcement, and high internal similarity scores lower than a concept with
low values in all categories.

Next check: validate against held-out human judgment; add calibration check.

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

### C-013: QA Confidence Can Be Calibrated Against Observed Accuracy

Status: `provisional`

Statement: The system can measure the gap between its stated confidence and its
actual accuracy by recording (confidence, was_correct) pairs from episode outcomes
and computing calibration metrics (ECE, calibration gap).

Owner modules: `src/cognition.rs` (`ConfidenceCalibration`), `src/qa.rs`.

Evidence: `ConfidenceCalibration` records per-bin accuracy and confidence,
computes ECE, identifies over/underconfidence.  Tests verify empty-state handling,
recording (perfectly calibrated data gives low ECE, overconfident data gives
positive calibration gap), and integration with `EpisodeStore` / `CognitiveEpisode`
outcomes.  5 unit tests pass.

Baseline: confidence was stated but never checked against reality.

Failure condition: the system reports overconfidence (avg_confidence >> accuracy)
or underconfidence (avg_confidence << accuracy) without detecting it.

Next check: wire `ConfidenceCalibration::record_store()` into the main agent loop
every 50 ticks so that calibration is tracked continuously across sessions.

### C-014: Feedback Improvement Can Be Measured Pre/Post

Status: `provisional`

Statement: The effect of feedback on a task family can be measured by running the
same questions before and after feedback, comparing accuracy and confidence deltas.

Owner modules: `src/cognition.rs` (`TaskFamily`, `PrePostComparison`).

Evidence: `TaskFamily` defines a set of task items (questions + verify-fact checks).
`TaskFamilyRun` captures results with per-task answers, confidence, and match
against expected.  `PrePostComparison` computes accuracy delta, confidence delta,
answer change count, and correctness change count.  Tests verify correct delta
computation for a simple before/after scenario.

Baseline: feedback effects were assessed only by reading printed logs.

Failure condition: a known improvement (e.g., storing the correct fact changes an
answer from wrong to right) is not reflected in the pre-post accuracy delta.

Next check: add a `QaEngine` integration test that defines a task family, records
pre-run, stores a new fact, records post-run, and verifies accuracy improvement.

### C-015: Theorem Proving via VSA Rewriting

Status: `provisional`

Statement: The VSA substrate can support bounded theorem proving, not through
search over proof trees (unification, resolution, AND-branching), but as
deterministic rewriting along causal chains encoded as bound hypervector
compositions.

Owner modules: `src/qa.rs` (causal-chain reasoning, `reason_chain()`),
`src/reason.rs` (forward chaining), `MATH.md` (Sub-Lemma S as linear causal
steps).

Current capability:
- **Causal-chain reasoning**: `reason_chain()` follows stored `IF A THEN B`
  rules forward from a known SVO fact, returning a sequence of (subj, verb, obj)
  up to 5 hops. Circular detection by max-hops bound.
- **Sub-Lemma S proof**: encoded as a deterministic linear sequence of
  ρ-admissible invariants (ρ¹³, ρ²⁶, ρ⁵²) and constructive witness geometry.
  **CLOSED for runtime-admissible manifolds (A3-Q).** The `enforce_a3q_manifold()`
  admission gate provides the quantitative decorrelation needed. The original
  "proven modulo decorrelation" gap is resolved by replacing an implicit
  probabilistic assumption with an executable theorem boundary.
- **Pure rewriting, no proof search**: the system does not branch on
  alternatives, backtrack, or unify terms. It applies rules in fixed order and
  accepts the first match.

Known gaps:
- **AND-branching**: No way to prove conjunctive sub-goals independently and
  combine results. A single `reason_chain()` is always linear.
- **Proof search / resolution / unification**: No unification of schematic
  variables, no refutation completeness, no proof-tree representation.
- **General theorem prover**: The VSA algebra lacks a sound inference calculus
  (no modus ponens rule for bound hypervectors, no substitution).

New in v3.4: governed reasoning verticals:
- **Proposition kernel**: 12-schema trusted environment with theorem
  instantiation, premise certificates, and replay verification (500-case seed:
  324/324 valid accepted, all 176 invalid rejected).
- **Recurrence vertical**: deterministic first-order affine recurrence solving
  with authorization, execution, and replay (500-case seed: 251/251 expected-
  positive executed/replayed, all 249 expected abstentions rejected).
- **Algebra benchmarks**: linear, quadratic, and 2×2 system executors with
  generated holdouts (560-case: 1.000 solution/execution/replay).

These are not theorem proving in the AND-branching sense; they are
deterministic executor-based verification, but they demonstrate bounded
formal reasoning at scale.

Baseline: pure string-pattern rewriting with no causal structure.

Failure condition: the system claims to prove a theorem that requires
AND-branching, unification, or proof search (e.g., ∀x P(x) → Q(x) with
multiple simultaneous instantiations).

Next check: add a test that distinguishes linear causal-chain rewriting from
true AND-branching proof (e.g., prove "if A and B then C" where A and B are
independent facts that must both be retrieved). Confirm the system correctly
fails or abstains.

### C-016: Strategy Guidance Can Reduce Route Cost Without Becoming Authority

Status: `supported`

Statement: A validated stored strategy can provide an auditable route-cost
counterfactual and guide an execution only after independent route
revalidation; the ordinary capability contract and replay verifier remain the
authority boundary.

Owner modules: `src/algebra_benchmark.rs`, `src/capability_planner.rs`,
`src/strategic_route_benchmark.rs`.

Evidence: the algebra strategy-shadow harness independently revalidates every
recommendation before calling the existing executor.  The 560-case generated
run revalidated 553/553 recommendations, saved 742 counterfactual steps, and
kept positive execution/replay at 1.000 with zero false authorizations or
denials.  The route-drift regression rejects a mutated stored route before
execution.  Contextual and global-only comparisons include an explicit mixed
evidence case (500 global successes versus one matching recent success), which
produces `ExploreFresh` at a support threshold of two.  The same boundary is
now exercised by expression-evaluation and substitution receipt tests; both
must pass their existing replay verifiers.
A separate `controls` fixture exercises the 2×2 system receipt and replay path
under the same route/revalidation contract.

Baseline: a stored strategy could be treated as an executable route, or global
support could be mistaken for local precedent.

Failure condition: a stale or drifted strategy bypasses independent
revalidation, changes positive execution/replay results, or produces a false
authorization/denial; contextual support inherits mismatched evidence.

Next check: extend the versioned receipt report with negative/stale route cases
and verify that all such candidates are rejected before execution.

### C-017: Concept Composition Is Bounded And Deterministic

Status: `supported`

Statement: Composition over validated concept contracts can be explored with
an explicit depth bound, deterministic traversal, and route deduplication
without executing or registering temporary composites.

Owner modules: `src/concept_composition_benchmark.rs`,
`src/capability_planner.rs`.

Evidence: the six-concept branching fixture reports zero routes at depth two,
eight three-fragment routes at depth three, and the same eight routes at depths
four and five because no longer typed route exists. Repeated evaluation is
byte-equivalent and all reported routes have zero planning rejections. A
five-stage, 4-way stress fixture (20 concepts, 1,024 routes) retained nested
deterministic frontier subsets across budgets 1/16/64/256/1024, with no
execution or registry mutation.

Baseline: unbounded composition search or treating temporary composites as
executable capabilities.

Failure condition: candidate growth exceeds the explicit depth/budget bound,
repeated traversal produces nondeterministic route sets, or a composed route
crosses the execution boundary without normal capability authorization.

Next check: only extend beyond the five-stage, 4-way baseline if a concrete
evaluation question requires larger graphs; the six-concept, 3×3×3, and
4-way five-stage probes already record visited nodes, pruned candidates,
deterministic frontier membership, and nested-budget behavior.

### C-018: Persistent Use Respects Declared Budgets

Status: `supported`

Statement: Sustained ingestion cannot grow cluster, entry, transient,
index, or conversation memory past the declared budget, and the reported
memory accounting includes every resident component rather than a single
favourable term.

Owner modules: `src/reliability.rs`, `src/lib.rs`, `src/chat_server.rs`.

Evidence: `MemoryBudget` is enforced in the real insertion paths
(`add_to_dejavu_db`, `novelty_gate_with_budget`, `add_transient_fact`) by
evicting rather than appending past the cap. `MemoryReport` /
`account_brain` / `account_conversation` count entries, metadata,
accumulators, centroids/anchors, associations, experiences, indexes, and
conversation state. A 20,000-observation sustained-ingestion run reports caps
respected and the accounted total within budget
(`docs/phase7_reliability_v1.md`). Unit tests drive thousands of insertions
against tight budgets and assert the caps hold.

Baseline: cluster count and transient entries grew without a budget; the only
runtime accountant counted cluster/entry counts plus a dense-accumulator
approximation and hard-coded several fields to zero.

Failure condition: under a stationary or slowly drifting stream, cluster
count, transient entries, or accounted bytes exceed the declared budget, or
the reported total omits a resident component.

Next check: extend the sustained-ingestion run to cover the conversational
ingestion path and to attribute index bytes per index family.

### C-019: Reported Guarantees Match The Active Configuration

Status: `supported`

Statement: Every reliability guarantee the system reports names its
enforcement status (checked / enforced / assumed) and the configuration under
which it holds, so a claim can never be read as stronger than the active
build supports.

Owner modules: `src/reliability.rs`, `src/chat_server.rs`,
`src/bin/machine_chat.rs`, `MATH.md`.

Evidence: `reliability::guarantee_registry()` records witness and
configuration per guarantee and is emitted to
`docs/phase7_reliability_v1.report.json`. Replay checks are classified as
hash consistency, recomputation, or independent verification
(`replay_catalogue`), and the projection telemetry names its measured path
(`CpuSoftProjection` in this build). The HTTP server enforces bounded request
bodies, a bounded in-flight turn count, turn execution timeouts, and a
loopback-default bind with bearer-token auth required before any non-loopback
exposure.

Baseline: replay was described by one word regardless of strength; projection
telemetry did not name its path; the server accepted an unauthenticated
non-loopback bind and had no request or execution bounds.

Failure condition: a guarantee is reported as enforced but no enforcement
point exists; a hash-consistency check is presented as independent
verification; or a non-loopback bind succeeds without authentication.

Next check: add a runtime endpoint that serves the current guarantee registry
so an operator can compare the deployed configuration against the report.

### C-020: Document-Derived Knowledge Is Inspectable And Reversible

Status: `supported`

Statement: Imported documents contribute knowledge only through an inspectable
propose-then-accept process; every extracted item (and every rejection) is
visible with its source location, committed knowledge cites its document, and
removing a document invalidates exactly the knowledge derived from it while
leaving independently taught knowledge untouched. Imported text is treated as
data: instruction-like sentences are never executed and can never be committed.

Owner modules: `src/document_learning.rs`, `src/persistence/documents.rs`,
`src/conversation/service.rs`, `src/chat_server.rs`, `src/bin/machine_docs.rs`.

Evidence: `document_learning::extract_*` records a byte span and page per item;
`classify_instruction` marks instruction-like text with `REASON_INSTRUCTION`;
`ConversationService::{import_*, inspect_document, accept_document_item,
commit_document, learn_document, remove_document}` persist and review proposals;
`persistence::documents::committed_assertion_ids` links each committed item to
its assertion so removal retracts exactly those. Acceptance tests in
`conversation_runtime_integration` import a document, ask before (abstains) and
after (answered, citable) commit, and remove it (abstains again), including
across a reload.

Baseline: documents could not be imported into the application at all; there was
no per-document provenance linking committed knowledge to its source and no way
to withdraw a document's influence.

Failure condition: an item reaches memory without explicit acceptance; an
instruction inside a document is executed or committed; removing a document
leaves its derived knowledge answerable or removes unrelated knowledge.

Next check: extend extraction to scanned documents with OCR, preserving the same
span-and-page provenance and the same propose-then-accept workflow.

### C-021: The Complete Application Is Assessed Automatically

Status: `supported`

Statement: A release is assessed automatically against representative
conversations, resource limits, and frozen evaluation cases. Reproducible
conversation traces are versioned regression cases; an untouched holdout set is
reported separately; and each mechanism (VSA retrieval vs a lexical baseline,
context retrieval, typed capabilities, reuse, the semantic worker, and
consolidation) is measured as a paired enabled/disabled run. Oracle correctness
of delivered answers is reported separately from the system's own self-rejection
(abstentions and clarifications), so neither is ever presented as the other.

Owner modules: `src/conversation_eval.rs`, `src/bin/machine_eval.rs`,
`src/conversation/service.rs` (`EvalConfig`), `data/conversation_eval_v1.json`,
`data/conversation_eval_holdout_v1.json`.

Evidence: `machine_eval` runs the corpora, captures and diffs traces
(`detect_drift`, normalized for volatile ids and latency), computes the metrics
(correctness, coverage, unsupported assertions, clarification success, context
accuracy, correction propagation, latency, memory), runs `run_ablations`, and
writes `docs/phase9_conversation_eval_v1.report.json` / `.md`. The committed run
scores regression 18/18 oracle-correct with 0 unsupported assertions and 0
drift, holdout 10/10, and six paired ablations that all preserve safety.

Baseline: conversations were exercised only by hand-written acceptance tests;
there was no corpus with gold labels, no trace replay, no aggregate metrics, and
no paired ablations, so a release could not be assessed automatically against
the complete application.

Failure condition: a delivered answer is counted as correct because the system
abstained; an unsupported assertion is delivered; a mechanism ablation changes
the corpus between its two sides or bypasses the safety contract; or the
holdout set is used to tune the system.

Next check: add adversarial and long-horizon conversations to the regression
corpus, and track per-capability coverage so a regression names the capability
that decayed.

### C-022: Background Work Is Bounded By A Declared Authority And Budget

Status: `supported`

Statement: The conversation can initiate useful background work, and that work
cannot exceed its declared authority or budget. Every task declares an
`AuthorityScope` (an allowlist of capabilities with no shell, network, or
simulation variant) and a `TaskBudget` (reusing `cognition::AutonomyBudget`, plus
a step cap) up front. The scope is enforced before each capability runs, so a
breach is a refusal rather than an overrun. Every task is observable (one
`TaskProgress` record per step), stoppable and resumable (`pause`/`resume`
continue from the next step), and can explain its result, its stop reason, and
whether it stayed within its declaration.

Owner modules: `src/autonomy_task.rs`, `src/bin/machine_eval.rs` (`task` mode),
`src/reliability.rs` (`G-TASK-AUTHORITY`, `G-TASK-OBSERVABLE`).

Evidence: `run_task_suite` / `run_task_scenarios` run the canonical suite
(analyze a document, investigate, run a selected experiment, consolidate
memory, produce a report, plus a paused-and-resumed analysis and a
budget-starved refusal) and report `TaskSuiteReport`; `machine_eval task` writes
`docs/phase10_autonomy_v1.report.json` / `.md` and exits non-zero if a task
exceeded its declaration or failed to explain its result. Unit tests cover
planning refusal, step-time budget refusal, pause/resume continuation,
cancellation, and the report. The integration test
`controlled_autonomy_tasks_run_stop_resume_and_explain` drives the same paths
against the live conversation, including a conversation-backed consolidation.

Baseline: conversation could not initiate any background work. There was no
task, scope, budget, progress, cancellation, or resume abstraction anywhere in
the runtime; `AutonomyBudget` existed but only bounded simulator actions, and
was unreachable from chat.

Failure condition: a capability runs outside the task's declared scope; a step
runs after the declared budget is exhausted; a task reports `Completed` without
a result, or reports a refusal as a completion; a resumed task repeats an
already-taken step; or a task kind can express shell, network, or simulation
authority.

Next check: allow a task to request a scope extension that a human must approve
before the blocked capability runs, and add long-running tasks whose progress is
persisted so a resume survives a process restart.

### C-023: Everyday Operation Uses One Command And Upgrades Are Recoverable

Status: `supported`

Statement: Installation, daily use, upgrades, and recovery no longer require
remembering research-specific commands. A single operator command (`machine`)
provides first-run setup, startup, configuration validation with a remedy per
problem, health and dependency status, a versioned capability inventory, release
notes tied to the committed evaluation artifacts, backup/restore, an upgrade that
takes a backup before migrating, and a recovery that restores the latest backup.
A database newer than the running build is refused rather than misread.

Owner modules: `src/operator.rs`, `src/bin/machine.rs`, `src/reliability.rs`
(`G-RELEASE-SCHEMA`), `src/persistence/db.rs` (newer-schema refusal),
`integration-tests/conversation_runtime/src/lib.rs`.

Evidence: `OperatorConfig::validate` returns `ConfigIssue`s (severity, field,
message, remedy); `doctor` returns a `DoctorReport` of ordered checks;
`CAPABILITIES` is the versioned inventory; `release_notes` reads the committed
`docs/phase7_reliability_v1.report.json`, `docs/phase9_conversation_eval_v1.report.json`,
`docs/semantic_fidelity_eval_v1.report.json`, and `docs/phase10_autonomy_v1.report.json`
and reports their measured numbers. `run_upgrade` refuses an incompatible
database before writing, backs up with SQLite's online backup API, then migrates
and verifies; `recover` restores the most recent backup. Unit tests cover
validation, profiles, inventory, setup, doctor, upgrade, refusal, backup/restore,
recover, and artifact writing; the integration test
`release_packaging_validates_health_inventory_and_upgrade_recovery` exercises the
same paths, including refusing a forged newer database without modifying it.

Baseline: operating the runtime meant remembering per-phase binaries
(`machine_chat`, `machine_docs`, `machine_eval`, `reliability_report`, ...), and
an upgrade had no documented pre-migration backup or rollback path.

Failure condition: a newer-than-build database is opened and migrated; an upgrade
migrates before taking a backup; `doctor` reports healthy while an enforced
guarantee or check fails; the capability inventory or release notes claim an
evaluation result not present in the committed artifacts; or recovery silently
proceeds without a backup.

Next check: package a signed release archive with checksums, and extend the
doctor to verify an installed binary against the inventory's declared artifacts.

## Retired Or Negative Claims

Negative results are useful research output.  Do not delete them just because
they are inconvenient.

- Assumption A21, abstraction preservation: current evidence says the existing
  abstraction map does not preserve task-relevant causal structure for held-out
  structural variants.
- Earlier soft-projection formula claims before the v3.1 correction: superseded
  by the corrected formula and calibration.
- Earlier `L_F <= 0.5` bound: superseded by the tighter `L_F <= 1.0` correction.
