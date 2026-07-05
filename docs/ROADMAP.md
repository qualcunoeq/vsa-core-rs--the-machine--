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
- `src/reason.rs`: projection, chaining, soft projection, theorem tests.
- `src/hnsw.rs`: approximate nearest-neighbor search for hypervectors.
- `src/compression.rs`: bitwise compression and coding experiments.

Research questions:

- Which operations preserve useful structure under noise?
- How much information survives bundling, binding, rotation, decay, and projection?
- Where does hard projection help, and where does soft projection preserve signal?

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

Near-term work:

- Track concept birth, merge, split, freeze, and decay events in a structured log.
- Add an ablation for abstraction enabled vs disabled.
- Define a concept quality score that does not depend on manual inspection.

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

Near-term work:

- Add traceable `resolve_term` results in QA.
- Store answer provenance as structured data, not only printed text.
- Maintain negative results where VSA structure does not improve a task.

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

Near-term work:

- Persist `AbstractionLearner` promotions with version metadata.
- Add confidence calibration checks for diagnostic and QA answers.
- Track pre/post feedback behavior on the same task family.

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

Near-term work:

- Add a tool-use event schema with input, output, confidence, and memory impact.
- Separate simulated actions from real external actions at the type level.
- Make tool reliability part of the self-model.

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

Near-term work:

- Prefer audit trails, integrity checks, sandboxing, and fail-closed behavior.
- Define an autonomy budget model: time, action count, resource use, risk.
- Require every external action to produce a replayable decision record.

## Research Hygiene

Use these conventions as the project grows:

- Default tests verify invariants and must stay deterministic.
- Ignored tests are benchmarks, calibrations, or dataset-backed validations.
- Every broad claim belongs in `docs/CLAIMS.md`.
- Every recurring experiment belongs in `docs/EVALUATION.md`.
- Failed hypotheses belong in the claim ledger as negative results, not in memory.

