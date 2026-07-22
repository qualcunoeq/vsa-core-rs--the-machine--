# Evaluation Matrix

This file defines how to measure progress without narrowing the project.  The
architecture can stay broad, but each capability should have a local check, a
baseline, and a failure mode.

## Test Classes

### Unit

Fast deterministic checks for invariants.  These run in the default suite.

Command:

```bash
cargo test --lib
```

Expected contents:

- algebraic identities;
- projection and accumulator invariants;
- parser and encoder edge cases;
- small deterministic reasoning examples.

### Calibration

Quantitative checks for approximate claims.  These may emit tables and may be
ignored by default.

Command pattern:

```bash
cargo test --lib reason::tests::test_soft_projection_frontier_sweep -- --ignored --nocapture
```

Expected contents:

- soft projection frontier sweeps;
- contraction and tracking estimates;
- merge/split probability calibration;
- seeded Monte Carlo checks.

### Benchmark

Long-running or dataset-backed measurements.  These are ignored by default and
should record inputs, seed, metric, and output.

Command pattern:

```bash
cargo test --lib -- --ignored
```

Expected contents:

- chess cross-validation;
- bond pipeline validation;
- large HNSW recall sweeps;
- full autonomy validation runs.

## Capability Matrix

| Capability | Baseline | Metric | Current surface | Next improvement |
| --- | --- | --- | --- | --- |
| Noise-stable memory | raw HV stream | NHD to source, retained signal, memory growth | `src/lib.rs`, `src/reason.rs` | structured long-run memory benchmark |
| Concept formation | similarity-only grouping | prediction gain, compression gain, concept churn | `src/abstractor.rs`, `src/cognition.rs` | `test_abstraction_ablation_benchmark` (ignored) emits `ExperimentResult` JSON |
| Temporal prediction | last-state or frequency baseline | top-k accuracy, calibration error | `src/temporal.rs`, `src/predictive.rs` | seedable transition benchmark |
| QA recall | direct lookup | answer accuracy, provenance completeness | `src/qa.rs`, `src/cognition.rs` | persist `CognitiveEpisode` records, including rule-level chain traces |
| Analogical transfer | non-VSA parser/classifier | held-out transfer accuracy | `src/analogy.rs` | keep negative A21 result until mechanism changes |
| Diagnostics | static keyword map | held-out diagnosis accuracy, category drift | `src/diagnostic.rs`, `src/abstraction_learner.rs` | persistent learner promotion audit |
| Feedback learning | no post-answer update | outcome score, reversible update rate, regression rate | `src/cognition.rs` | append-only feedback store |
| Tool use | direct invocation logs | replayability, reliability estimate, side-effect class | `src/action.rs`, `src/actuator.rs`, `src/cognition.rs` | route actuator calls through `ToolEvent` |
| Autonomy | unconstrained loop | success rate under budget, rollback rate, unsafe-action blocks | `src/bin/autonomy_experiment.rs`, `src/cognition.rs` | require external actions to spend `AutonomyBudget` |
| Resilience | no adversary | recovery time, false positive rate, integrity preservation | `src/defense.rs`, `src/monitor.rs` | operator-visible resilience tests |
| Ablation | all mechanisms enabled | delta vs full model by capability metric | `src/cognition.rs` | run QA and abstraction with explicit `AblationConfig` |

## Result Record

Each recurring experiment should produce enough information to compare runs.
Use this shape in logs or serialized output:

```json
{
  "experiment": "name",
  "claim": "C-000",
  "commit": "git-sha",
  "seed": 0,
  "dataset": "none-or-path",
  "baseline": "description",
  "metrics": {
    "primary": 0.0
  },
  "passed": true,
  "notes": "short human-readable summary"
}
```

The Rust-side carrier for this schema is `cognition::ExperimentResult`.  New
benchmarks should prefer emitting that structure as JSON over printing
human-only tables.

## Cognition Benchmark Runner

The future rented-machine benchmark campaign should use:

```bash
cargo run --release --bin cognition_bench -- --case all --scale large --seed 42 --out results/cognition_bench/run.jsonl
```

The runner defaults to `small` scale and `/tmp/cognition_bench.jsonl`, so local
use stays limited unless a larger scale is explicitly requested.

### Transformer-Cousin Behavioral Suite

Use this as the first behavioral reference point for comparing the bitwise
architecture against transformer-style expectations without calling an LLM:

```bash
cargo run --release --bin cognition_bench -- --case transformer-cousin --scale small --seed 42 --out results/cognition_bench/cousin.jsonl
```

The suite measures:

- grounded QA accuracy over explicit facts;
- multi-hop chain accuracy over causal rules;
- abstention on unknown questions;
- improvement after feedback insertion;
- term-trace coverage for explanation/provenance.
- rule-level chain-trace coverage, replay distance, and explicit chain termination
  for multi-hop causal answers.

The primary summary metric is `aggregate_score`, with component metrics emitted
alongside it in the standard `ExperimentResult` JSONL schema.

## Promotion Rules

A mechanism can move closer to core architecture when:

- it beats its baseline on a seeded check;
- it has at least one default deterministic invariant test;
- it has a documented failure condition;
- it does not require manual log inspection to know whether it worked.

A mechanism should stay experimental when:

- it only works in a single hand-built scenario;
- its success depends on broad printed output;
- it has no baseline;
- it improves one domain while silently degrading another.

## Immediate Evaluation Debt

- Convert print-only calibration tests into assertions on coarse, defensible
  ranges, or move them into explicit benchmark runners.
- Add structured output to ignored benchmark tests.
- Reduce warning noise in core modules so new warnings are visible.
- Record negative results in `docs/CLAIMS.md` before removing failed mechanisms.

## Strategic Route Evaluation

The first vertical-slice harness for the concept/strategy layer is deliberately
planning-only. It evaluates four independently reported modes plus a receipt
shadow over deterministic
typed tasks: direct capability planning, concept-guided proposals, stored
strategy reuse, and full contextual exploration/exploitation diagnostics.

```bash
cargo run --release --bin strategic_route_bench -- \
  --scale medium --seed 42 --out results/strategic_route_bench/run.jsonl
```

Scales are `small` (32), `medium` (256), and `large` (500) tasks. Each JSONL
record is an `ExperimentResult` with planning accuracy, abstention,
concept-retrieval, stored-strategy usefulness, contextual-retrieval, and
false-authorization metrics. Failure taxonomy counts are emitted as stable
`failure_<class>` metrics. These are route-oracle tasks, not solved-answer
claims; execution and verification remain outside the benchmark.
The report also includes a contextual-support ablation: the same frontier is
evaluated with matching contextual evidence and with global support only. On
the medium seed, contextual guidance is correct on all 198 context-sensitive
tasks, while global-only support produces wrong decisions on a nonzero subset.
The benchmark suite also has an explicit mixed-evidence regression: a stored
strategy with 500 global successes but only one matching recent contextual
success yields `ExploreFresh` at a support threshold of two; the global-only
comparison remains `Ambiguous` rather than silently treating global volume as
local precedent. The medium run at seed 42 reports contextual accuracy 1.000
versus 0.146 for the global-only ablation (169 wrong global-only decisions),
with zero false authorizations. The focused capability-planner suite exposes
the five adversarial context dimensions as independent tests: domain,
contract-signature, policy-class, stale epoch, and safety-only evidence. Each
keeps global support high while reducing contextual support to zero and
diagnosing `ExploreFresh`.
The `strategic_receipt_shadow` record covers expression evaluation,
substitution, and the `controls` system fixture; each route must be
independently revalidated before the existing executor/replay verifier runs.

The formalization baseline now emits the same dominant-failure taxonomy in its
report. Run it against a versioned corpus with:

```bash
cargo run --release --bin formalization_baseline -- \
  data/formalization_seed_v1.json results/formalization_seed_report.json
```

The report records one primary class per incorrect outcome, classification
coverage, and the original blocker evidence. Correct abstentions are not
counted as failures; false authorization is always classified as a safety
failure.

The current typed direct-instantiation audit also uses complete fact
provenance when deciding readiness. Explicit equation solves, arithmetic
expression requests, and supported function applications are admitted only
when their typed subject and bindings are complete; derived-property requests,
unmodeled domain restrictions, and model-dependent applications remain denied.
The constrained prose grammar additionally recognizes a bounded set of
equations, rates, inequalities, systems, quantifiers, units, and entity
relations. On `data/formalization_seed_v1.json` (60 cases), the audit reports
`authorization_correct=60`, `false_authorizations=0`, `false_denials=0`, and
full failure-taxonomy coverage; executable target completeness is 47/60 while
structural target completeness is 60/60 with no structural target failures.
The field-level grammar now reaches
definitions 21/21, facts 69/69, entities 21/21, assumptions 3/3, constraints
9/9, and obligations 35/35 on the reviewed seed (with precision reported
separately for conservative over-extraction). Verification-intent recognition covers all 60
seed prompts; relation, predicate, set-membership, and explicit affine
recurrence requests are represented as typed, provenance-bearing targets.
Recurrence targets remain behind the specialist gate and do not authorize the
generic evaluator. Structural completeness is reported separately from
executor/verifier availability, so a fully typed unsupported operation does not
become an authorization.

For machine-readable tier/holdout metrics, use the standard JSONL runner:

```bash
cargo run --release --bin formalization_bench -- \
  data/formalization_seed_v1.json results/formalization_bench/seed.jsonl
```

In addition to structural and authorization metrics, this runner reports a
diagnostic planning bridge for every complete typed target:
`planning_success_rate`, `planning_none`, `planning_ambiguous`, and
`planning_dependency_failure`.  A successful bridge means that the governed
capability planner can expand a selected method into a dependency-first plan;
it does not authorize execution or change the conservative direct-audit gate.

The first execution-level vertical slice is the bounded algebra benchmark:

```bash
cargo run --release --bin algebra_bench -- \
  data/algebra_seed_v1.json results/algebra_bench/seed.jsonl
```

It runs the real linear-equation, quadratic-equation, and 2×2 linear-system
executors, then replays each successful receipt.  The JSONL report separates
formalization, method selection, execution, replay, exact-result accuracy,
route length, false authorization, and false denial.  Adversarial cases are
expected to abstain; they are not scored as failed solves.
Every corpus-provided `tier` is also emitted as a `tier:<name>` group, so
development, holdout, generated, and adversarial performance can be compared
without mixing their denominators.

To add deterministic parameterized cases without changing the versioned seed:

```bash
cargo run --release --bin algebra_bench -- \
  data/algebra_seed_v1.json results/algebra_bench/generated.jsonl \
  HEAD 200 42
```

The generated gold answers come from integer witnesses and are independent of
the executor; every fifth generated case is held out.

For a small language-shift slice, run the prose corpus separately:

```bash
cargo run --release --bin algebra_bench -- \
  data/algebra_prose_v1.json results/algebra_bench/prose.jsonl
```

This corpus contains 20 natural-language linear, quadratic, and system
requests, including four held-out prompts and adversarial abstention cases.
The current slice executes every authorized case with exact results and replay
verification, while recording zero false authorizations and zero false
denials.

The same runner also emits `algebra_strategy_shadow`.  This is the staged
strategy-integration check: a stored route is compared with a fresh route,
independently revalidated against the current registry, and then sent through
the ordinary executor/replay verifier.  Strategy guidance remains diagnostic;
the stored route never authorizes execution by itself.  The generated 260-case
run revalidated all 253 eligible recommendations, replayed every successful
execution, and saved 342 counterfactual capability steps with zero false
authorizations or denials.

The positive-only shadow metrics form an explicit ablation against the
ordinary algebra baseline: both modes must retain identical positive execution
and replay rates. Any claimed benefit is therefore confined to counterfactual
route cost, preventing a strategy from claiming an accuracy gain when it only
repackages the same governed executor.

The next execution-level vertical slice is the bounded recurrence benchmark:

```bash
cargo run --release --bin recurrence_bench -- \
  500 42 results/recurrence_bench/large.jsonl HEAD
```

This runner generates deterministic first-order explicit-affine recurrence
cases and evaluates the existing typed executor directly. It reports total,
development, and every-fifth holdout slices separately, with authorization,
execution, replay, false-authorization, false-denial, and refusal-taxonomy
metrics. The negative cases cover missing and conflicting initial conditions,
unroll limits, domain and base-index violations, and checked arithmetic
overflow; none may cross the execution boundary. On the 500-case seed-42 run,
251/251 expected-authorized cases executed and replayed, all 249 expected
abstentions were rejected, and the holdout slice retained 50/50 positive
execution/replay with all six refusal classes represented. There were zero
false authorizations and zero false denials.

The elementary discrete/proof vertical uses the trusted proposition kernel
directly:

```bash
cargo run --release --bin proposition_bench -- \
  500 42 results/proposition_bench/large.jsonl HEAD
```

It generates theorem instantiations, premise-bearing symmetry/transitivity,
universal introduction, and certified arithmetic proofs, then adds malformed
proof objects for missing binders, invalid certificates, missing premises,
unknown theorem IDs, and wrong conclusions. Every accepted proof is checked a
second time for replay. On the 500-case seed-42 run, 324/324 expected-valid
proofs were accepted and replayed; all 176 expected-invalid proofs were
rejected, with zero false acceptances or false rejections. The 100-case holdout
retained 65/65 valid accept/replay results and represented all five refusal
classes. This is a proof-kernel baseline, not theorem discovery: the trusted
environment remains the 12 curated initial schemas. The unified suite's
verification control is receipt-level and diagnostic; it does not execute a
verification-off authorization path.

The unified governed suite aggregates these verticals into explicit evaluation
tiers and records which ablations are actually implemented:

```bash
cargo run --release --bin governed_bench -- \
  500 500 42 results/governed_bench/large.jsonl 61d7b58
```

The seed-42 run reports seven tiers. Tier 0 direct algebra execution is 27/27
accepted and replayed; tier 1 prose formalization is 0.500 accepted over its
20-case corpus with replay rate 1.000; tier 2 proposition proofs are 324/500
accepted and 1.000 replayed; tier 3 strategic method selection is 500/500
correct with receipt-shadow replay rate 1.000; and the 21-case adversarial
algebra tier has zero false authorizations and denials. The recurrence total is
251/500 authorized and replayed, with the 100-case holdout at 0.500 acceptance
and 1.000 replay. The report now emits both aggregate rates and explicit
positive-case success/replay rates; `expected_positive` remains explicit so
abstention-heavy tiers are not mistaken for positive-case accuracy.

The suite evaluates only concrete controls: strategy-memory,
concept-memory, contextual-support, proof-reuse, fact-reuse, and verification
ablations are measured. The proof slice records 50/50 indexed hits with 50/50
replay-verified results; the fact slice records 50/50 governed retrieval hits
with 50/50 retrieval receipts. The verification control uses valid and forged
linear-equation receipts: the production replay gate accepts every valid case
and rejects every tampered case, while the explicitly unsafe no-verification
counterfactual false-accepts every tampered receipt. No unsafe executor or
registry path is introduced; the bypass is a diagnostic calculation only.

The runner appends a `governed_suite_runtime` result rather than hiding
performance in console output. Release 500/500 runs measured about 1.0–1.1 s
on the development host (seed 42, seven tiers); this is a recorded baseline,
not an asserted universal SLO.

The concept-composition resource probe measures bounded DFS growth without
executing or registering any composed route:

```bash
cargo run --release --bin concept_composition_bench -- \
  5 results/concept_composition.json
```

The command also writes a larger branching budget sweep beside the requested
report (by default, `results/concept_composition.json.budget.json`). The
default sweep uses five typed stages with four alternatives per stage: 20
validated concepts and 1,024 full routes. Budgets 1, 16, 64, 256, and 1,024
retained exactly that many proposals; every partial result was a deterministic
subset of the full frontier, nested as the budget increased, and remained
diagnostic-only. A smaller 3×3×3 sweep remains covered by the library tests.

The six-concept branching fixture produced 0 candidates at depth 2 and 8
three-fragment candidates at depth 3; depths 4 and 5 remained at 8 because no
longer compatible route exists. The report records proposal/rejection counts,
route-length histograms, a conservative theoretical path bound, and
deterministic replay of the measurement itself. It also runs a diagnostic
candidate budget of four and records proposals retained, search nodes visited,
candidates pruned, and whether a budget equal to the full proposal count
preserves the complete frontier. The budget only bounds diagnostics: it does
not execute, authorize, or register a composed route.

Latest larger-tier smoke runs (commit `f311a23`) used 500 generated cases in
addition to the 60-case seed (560 total) and the 20-case prose slice. The
generated run retained 1.000 solution, execution, and replay rates with zero
false authorizations/denials; its shadow revalidated 553/553 recommendations,
saved 742 counterfactual steps, and kept positive execution/replay at 1.000.
The prose run retained 1.000 execution/replay and zero false authorizations or
denials (formalization was 0.950 because one adversarial prompt correctly
abstained). The large 500-task strategic run retained 1.000 accuracy in all
four modes; contextual guidance was correct on every context-sensitive task,
while the global-only ablation was wrong on 332 of them.
On the current workstation, the release binaries completed the 500-task
strategic run in below the shell timer's 0.01s resolution (about 2.6MB peak
RSS) and the 560-case algebra run in 4.45s (about 5.9MB peak RSS). The
adversarial algebra tier intentionally reports zero execution/replay attempts;
its exact-solution accuracy remains 1.000 because safe abstention is scored as
the correct outcome.
