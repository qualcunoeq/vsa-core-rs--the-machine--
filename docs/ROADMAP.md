# Research Roadmap

This project is intentionally broad: the long-term target is a bitwise cognitive
architecture that can perceive, reason, answer, adapt, improve, use tools, and
defend its own operating envelope.  The roadmap below does not narrow that
target.  It separates the target into layers so each capability can be improved
without losing the whole-system ambition.

## Guiding Principle

Build a broad architecture from narrow, falsifiable experiments.

Every new capability should enter the system with:

- a claim it is supposed to make true;
- a baseline it must beat or a failure mode it must reduce;
- a metric that can be measured without reading logs by hand;
- a small default test and, when needed, a larger ignored benchmark.

## Layer 0: Bitwise Substrate

Purpose: provide the stable symbolic substrate that all higher cognition uses.

Current anchors:

- `src/lib.rs`: `Hypervector`, `MemoryCluster`, accumulator dynamics, telemetry.
- `src/cognition.rs`: shared episode, feedback, ablation, budget, and result
  records used by higher layers.
- `src/reason.rs`: projection, chaining, soft projection, theorem tests.
- `src/hnsw.rs`: approximate nearest-neighbor search for hypervectors.
- `src/compression.rs`: bitwise compression and coding experiments.

Research questions:

- Which operations preserve useful structure under noise?
- How much information survives bundling, binding, rotation, decay, and projection?
- Where does hard projection help, and where does soft projection preserve signal?

Implemented surface:

- Shared `ExperimentResult` records can carry claim, commit, seed, baseline,
  metrics, and pass/fail state.

Near-term work:

- Keep deterministic tests for projection and accumulator invariants.
- Move long calibration sweeps behind `#[ignore]` with stable output schemas.
- Reduce warning noise in this layer first.

## Layer 1: Memory And Concept Formation

Purpose: turn streams into stable concepts without a training phase.

Current anchors:

- `src/abstractor.rs`: community detection and L2 concept formation.
- `src/hierarchy.rs`: multi-level projection and abstraction.
- `src/temporal.rs`: transition statistics and temporal prediction.
- `src/sleep.rs`: consolidation and pruning.

Research questions:

- Do clusters remain stable under non-stationary input?
- Can L2 concepts form from transition structure rather than superficial similarity?
- What should be forgotten, frozen, or promoted?

Implemented surface:

- `AblationConfig` can record whether trace, abstraction, associations, soft
  projection, self-model, and tool-memory mechanisms were enabled.

Implemented:

- `ConceptEvent` / `ConceptJournal` in `cognition.rs` — structured lifecycle log with
  push, query, and JSON persistence. Wired into abstractor cycle (creation, reinforcement,
  dissolution, decay) and cluster compactor (merge).
- `ConceptQualityScore` in `cognition.rs` — composite score from coherence, component
  count, freshness, and internal similarity. No manual inspection needed.
- `test_abstraction_ablation_benchmark` (ignored) — compares concept count and prediction
  error with abstraction on vs off, emits `ExperimentResult` JSON.

Near-term work:

- Wire `ConceptJournal` into `freeze_cold_clusters` (cold storage events).

## Layer 2: Reasoning And Explanation

Purpose: make the system answer questions and expose the path that produced an
answer.

Current anchors:

- `src/qa.rs`: question answering and causal-chain queries.
- `src/analogy.rs`: role-frame induction and analogical prediction.
- `src/reason.rs`: forward chaining and trajectory evaluation.
- `src/narrative.rs`: narrative state and explanation-facing structures.

Research questions:

- Can the system distinguish recall, inference, analogy, and speculation?
- Can it explain which memory, rule, or tool path produced an answer?
- Does analogical structure transfer across domains?

Implemented surface:

- `answer_episode()` wraps `answer()` with `ResolveTrace` provenance and auto-push
  to `episode_store`.
- `verify_fact_episode()` wraps `verify_fact()` with term-level traces.
- `answer_combined_episode()` and `answer_chain_episode()` (existing) cover combined
  and chain paths.
- Socket `ASK` and `CHAIN` commands now use episode wrappers, recording every
  admin-socket query in the episode store.
- Episode store automatically persisted every 50 ticks in `main.rs`.
- `traces_for_question()` collects term-level `ResolveTrace` for any question.

Near-term work:

- Wire multi-hop chain provenance: `reason_chain()` should return rule-level traces
  alongside text results.
- Add forward-chain / abduce episode wrappers for completeness.
- Wire `ConfidenceCalibration::record_store()` into the main agent loop every 50 ticks
  to track calibration over time.

## Layer 3: Self-Model And Adaptation

Purpose: let the architecture monitor its own competence, uncertainty, and
failure modes.

Current anchors:

- `src/self_model.rs`: self-state and mode tracking.
- `src/drift.rs`: cognitive drift and regulator experiments.
- `src/abstraction_learner.rs`: self-extending diagnostic categories.
- `src/diagnostic.rs`: failure classification and diagnosis paths.

Research questions:

- Can the system know when it does not know?
- Can it improve after feedback without erasing old competence?
- Can learned categories remain versioned, reversible, and auditable?

Implemented surface:

- `AbstractionLearner` promotions now include schema `version` (u64) and per-mapping
  `promoted_at_episode` + `metadata` HashMap for forward-compatible attribute extensions.
- `ConfidenceCalibration` in `cognition.rs` records (confidence, was_correct) pairs,
  computes ECE, calibration gap, and per-bin accuracy. Wired to `CognitiveEpisode`
  outcomes via `record_episode()` and `record_store()`.
- `TaskFamily` / `PrePostComparison` in `cognition.rs` define before/after feedback
  tracking: define a family of questions, run pre-feedback, apply updates, run post,
  and compare accuracy/confidence deltas.

Near-term work:

- Wire `ConfidenceCalibration::record_store()` into the main agent loop (every 50 ticks).
- Add a `run_pre_post()` integration test using `QaEngine`.
- Persist `SelfModel` state for cross-session trajectory continuity.

## Layer 4: Tools And World Interfaces

Purpose: connect cognition to files, code, web data, VMs, APIs, and other
external state while preserving auditability.

Current anchors:

- `src/action.rs`: tool registry and intent decoding.
- `src/actuator.rs`: action execution surface.
- `src/code_bridge.rs`: code ingestion and structural signatures.
- `src/sensory.rs`, `src/observer.rs`, `src/forager.rs`: input surfaces.

Research questions:

- Which tool outputs can be grounded into stable memory?
- Can the system learn tool reliability over time?
- Can action selection be explained and rolled back?

Implemented surface:

- `ToolEvent` / `ToolEventStore` — append-only audit log with JSON persistence,
  query by action type, success rate aggregation.  `ToolEventStore` stored on
  `VSABrain.tool_event_store` and wired into `run_attack_loop` via
  `record_tool_event()`.
- `SimulationMode` enum (`Real` / `Simulated`) added to `ActionRequest` as a
  first-class type-level field.  `ActionRequest::new()` defaults to `Simulated`;
  all helper methods (`.scan_port()`, `.check_service()`, etc.) use `.real()`.
- `ToolReliabilityTracker` — per-action-type EWMA success/failure tracking,
  stored on `VSABrain.tool_reliability`.  Updated alongside every tool event.
  Supports `success_rate()`, `reliability()` (EWMA), `overall_reliability()`.
  Case-insensitive action type lookup.

Near-term work:

- Wire `record_tool_event()` into the remaining execution paths (meta_reasoning,
  autonomy_experiment, main.rs legacy action path).
- Add a `tool_reliability` integration test that verifies EWMA updates across
  multiple real-tool calls.

## Layer 5: Bounded Autonomy And Resilience

Purpose: explore autonomous operation under explicit operator-visible
boundaries.

Current anchors:

- `src/bin/autonomy_experiment.rs`: simulated autonomy tasks.
- `src/bin/validate_autonomy.rs`: validation experiments.
- `src/monitor.rs`: monitoring state.
- `src/defense.rs`: threat detection and defensive reactions.
- `src/workspace.rs`: attention and workspace control.

Research questions:

- Can the system pursue goals while respecting budgets and constraints?
- Can it recover from tool failure, bad memory, or adversarial input?
- Can it protect integrity without hiding behavior from the operator?

Implemented surface:

- `AutonomyBudget` on `VSABrain.autonomy_budget` — enforced in `run_attack_loop`
  via `budgeted_execute()` which checks `can_spend()` before execution, calls
  `spend()` after, and records a `DecisionRecord` with full pre/post budget
  snapshots, reasoning, and link to ToolEvent.  Budget defaults: 1000 actions,
  1 hour, 100 external writes, 0.80 max risk.
- `DecisionRecord` / `DecisionJournal` on `VSABrain.decision_journal` — captures
  tick, intent, action request, result, budget before/after, reasoning, budget
  status, and ToolEvent link.  Persisted via JSON save/load.  Supports querying
  blocked records and successful records.

Wired into:
- `solve_autonomously_with_learner()` in `meta_reasoning.rs` — plan steps use
  `budgeted_execute()` instead of raw `send_request()`.
- `resolve_uncertain()` and `resolve_stuck()` in `meta_reasoning.rs` — hypothesis
  testing and documentation acquisition gated by budget.
- `main.rs` corrective plan execution — budget check before `execute_action()`
  with full `DecisionRecord` creation, spending, and logging.

Remaining work:
- Add `decision_journal.save()` to periodic persistence in main.rs (every 50 ticks).

## Layer 6: Governed Reasoning Evaluation

Purpose: evaluate planning, execution, strategy reuse, and contextual evidence
as one auditable vertical slice before broadening the architecture.

Current anchors:

- `src/strategic_route_benchmark.rs`: deterministic direct, concept-guided,
  stored-strategy, and full planning modes with a 12-bucket failure taxonomy.
- `src/concept_composition_benchmark.rs`: bounded DFS resource probe over a
  branching validated-concept graph; diagnostic-only and deterministic.
- `src/algebra_benchmark.rs`: versioned linear, quadratic, and 2×2-system
  corpora with generated holdouts, prose prompts, adversarial abstentions, and
  strategy-shadow execution metrics.
- `src/recurrence_benchmark.rs`: deterministic bounded first-order affine
  recurrence vertical slice with development/holdout execution and replay,
  adversarial refusal cases, and stable failure taxonomy.
- `src/proposition_benchmark.rs`: deterministic trusted proposition-kernel
  vertical with theorem instantiation, premise certificates, replay, and
  malformed-proof refusal taxonomy.
- `src/governed_benchmark.rs` and `src/bin/governed_bench.rs`: unified
  seven-tier report over direct execution, light formalization, proposition
  proofs, strategic selection, adversarial refusal, and recurrence slices;
  emits explicit evaluated versus unevaluated ablation status.
- `docs/EVALUATION.md`: reproducible commands, tier denominators, ablations,
  and recorded large-tier results.

Current evidence (commit `406ce80`):

- 500 strategic tasks: all four modes retain 1.000 planning accuracy; the
  context-aware/global-only ablation is correct on every context-sensitive task
  versus 332 global-only wrong decisions.
- 560 algebra cases (60 seed + 500 generated): exact solution, execution, and
  replay rates are 1.000 with zero false authorizations and denials.
- Strategy shadow: 553/553 recommendations independently revalidated, 742
  counterfactual steps saved, and positive execution/replay remain 1.000.
- Concept composition: a six-concept branching fixture produces eight valid
  three-fragment routes at depth three; deeper bounds do not add routes, and
  all measurements are deterministic with zero planning rejections.
- Resource stress: a five-stage, 4-way fixture (20 concepts, 1,024 routes)
  retained deterministic nested frontier subsets at budgets 1/16/64/256/1024;
  the release probe completed in about 0.04s at roughly 4MB RSS.
- Mixed contextual evidence: global support 500 versus contextual support 1,
  with a cheaper fresh route, correctly diagnoses `ExploreFresh`; global-only
  support remains `Ambiguous`.
- Recurrence vertical: the 500-case seed-42 run authorizes and replays 251/251
  expected-positive cases, rejects all 249 expected abstentions, and reports
  zero false authorizations and zero false denials. The 100-case holdout has
  50/50 positive execution/replay and represents all six refusal classes.
- Proposition-kernel vertical: the 500-case seed-42 run accepts and replays
  324/324 valid proof objects, rejects all 176 malformed proofs, and reports
  zero false acceptances and zero false rejections. The 100-case holdout has
  65/65 valid accept/replay results and covers all five refusal classes.
- Unified governed suite: the seed-42, 500/500 run reports seven explicit
  tiers. Direct algebra is 27/27 accepted/replayed; proposition proofs are
  324/500 accepted with replay rate 1.000; strategic method selection is
  500/500 correct with receipt-shadow replay rate 1.000; recurrence is
  251/500 authorized/replayed with 50/100 positive holdout execution/replay;
  and the 21-case adversarial tier has zero false authorizations or denials.
  Strategy-memory and contextual-support ablations are evaluated; concept,
  proof, fact, and verification-off ablations remain explicitly
  `not_evaluated` because no isolated end-to-end controls exist.
- Operational baseline: the release unified runner records a
  `governed_suite_runtime` receipt; the seed-42 500/500 run measured about
  1.04 s (roughly 1,043 ms) on the development host. Runtime is measured as evidence, with no
  unvalidated performance target inferred from one machine.
- Formalization audit: complete fact provenance now gates typed direct
  instantiation; the constrained prose grammar now covers bounded equations,
  rates, inequalities, systems, quantifiers, units, and entity relations. The
  60-case seed reports authorization correctness 60/60, zero false
  authorizations, zero false denials, complete failure-taxonomy coverage, and
  executable target completeness 46/60 (up from 37/60), prose-tier
  completeness improving from 1/20 to 10/20, and structural target
  completeness 56/60. Structural completeness is reported separately from
  executor/verifier availability.

Known limits:

- The strategy shadow is a governed ablation, not direct execution of a stored
  strategy; the existing method-specific executor remains the authority.
- Algebra cases are deterministic and narrow.  Generated holdouts test scale
  and parsing variation, not broad mathematical generalization.
- Context support currently requires exact domain, contract, policy, and recent
  epoch matches.  This is safe but may be sparse for transfer.
- The recurrence vertical is intentionally narrow: it evaluates supplied
  first-order explicit-affine definitions and does not infer closed forms,
  nonlinear dynamics, or missing methods.
- The proposition vertical benchmarks the trusted 12-schema environment and
  kernel checking; it does not claim automated theorem discovery or broad
  discrete-mathematics coverage. Verification-off ablations remain excluded by
  design because they would test an unsafe execution mode.

Next evaluation gate:

1. Treat the unified seven-tier suite, including the five-stage concept sweep,
   500-case recurrence run, and 500-case proposition-kernel run, as the current
   resource and execution baseline; pursue larger tests only when a concrete
   evaluation question requires them.
2. Consider governed method acquisition only after a benchmark records a
   demonstrated `method_not_found` capability gap. The current algebra,
   formalization, strategic, and recurrence slices provide no such evidence;
   absence of that evidence is not authorization to add a method.

## Research Hygiene

Use these conventions as the project grows:

- Default tests verify invariants and must stay deterministic.
- Ignored tests are benchmarks, calibrations, or dataset-backed validations.
- Every broad claim belongs in `docs/CLAIMS.md`.
- Every recurring experiment belongs in `docs/EVALUATION.md`.
- Failed hypotheses belong in the claim ledger as negative results, not in memory.
