# Research Lessons From The 2026-07-07 Benchmark Campaign

This document summarizes what the rented-server benchmark campaign taught us
about the current architecture. It is based on the committed reports and raw
artifact archive:

- `docs/benchmark_runs/final_combined_remote_20260707.md`
- `docs/benchmark_runs/remote_finalization_20260707.md`
- `docs/benchmark_runs/artifacts/remote_raw_artifacts_20260707.tar.gz`

## Campaign Scope

The campaign combined recovered historical results, chess self-play, and the
new failure-seeking cognition suites.

- Cognition rows analyzed: `3440`
- Cognition passed: `3300`
- Cognition failed: `140`
- Parse errors: `0`
- Chess rows analyzed: `8251`
- Chess games: `82381`
- Chess plies: `10309642`

Four remote cognition batches were finalized:

| Run | JSONL files | Exit 0 | Timeouts | Purpose |
| --- | ---: | ---: | ---: | --- |
| `next_gen_remote_20260707T081200Z` | 52 | 52 | 40 | hard adaptation, adversarial QA, latency SLO |
| `aux_pressure_20260707T082446Z` | 36 | 36 | 0 | memory pressure, QA depth, meta/temporal/autonomy pressure |
| `topup_fast_20260707T091631Z` | 96 | 96 | 0 | faster repeated hard/adversarial/latency trials |
| `final_fill_20260707T094608Z` | 80 | 80 | 0 | final useful fill while long-tail jobs finished |

The first important lesson is methodological: the old benchmark family mainly
proved that the program can run and return correct answers on clean synthetic
tasks. The new benchmark family is more useful because it produces stable,
interpretable failures.

## High-Confidence Findings

### 1. Basic Synthetic Correctness Is Not The Bottleneck

Most original suites still pass at `1.0000` average accuracy:

- `memory-pressure`: `537/537` passed
- `qa-depth-*`: all listed depths passed
- `adaptation`: `92/92` passed
- `meta-reasoning`: `376/376` passed
- `autonomy-budget`: `350/350` passed
- `temporal-abstraction`: `62/62` passed

This does not mean the architecture is generally intelligent. It means the
clean synthetic tests are too aligned with the implementation. They are still
useful as regression tests, but they are weak as intelligence tests.

### 2. Retrieval Precision Is The Primary Cognitive Weakness

The adversarial QA suite failed every completed row:

- `adversarial-qa`: `36` rows, `0` passed, `36` failed
- `false_positive_rate`: `1.0000` across the recorded metric range
- `exact_accuracy`: `1.0000`
- `answer_accuracy`: `1.0000`

This combination is important. The system can retrieve the intended positive
fact, but it also accepts wrong near-miss facts. That points to an overly
permissive verifier or matcher rather than a total inability to store facts.

The practical conclusion: before adding broader reasoning features, the memory
system needs stricter negative verification, exact-key paths, contradiction
handling, or calibrated thresholds for near-collision facts.

### 3. Adaptation Looks Correct Only Because It Is Too Permissive

The hard adaptation suite also failed every completed row:

- `hard-adaptation`: `56` rows, `0` passed, `56` failed
- `accuracy`: `1.0000`
- `before_false_positive_rate`: `0.9950`
- `near_miss_rejection`: `0.0000`
- `regression_rate`: `0.0000`

The system recalls inserted facts, but it behaves as if many facts were already
true before training and cannot reject near-miss decoys. That means the current
adaptation signal is not clean enough to prove learning.

The practical conclusion: future learning tests must measure all of these at
the same time:

- low pre-training false positives
- high post-training exact recall
- high near-miss rejection
- low regression on earlier facts

Passing only post-training recall is not enough.

### 4. Latency Scaling Is The Main Engineering Bottleneck

The latency SLO suite exposed correct-but-too-slow behavior:

- `latency-slo-memory-pressure`: `48` rows, `0` passed, `48` failed
- memory-pressure p95 latency: about `5,447,111 ms`
- latency-SLO memory-pressure p95 latency: about `4,866,179 ms`
- `slo_ratio` p95: `56.4578`
- `slo_ratio` max: `501.9954`

The system often finds correct answers eventually, but the retrieval path does
not scale to large memory. This is a stronger blocker than raw answer accuracy,
because a lifelong cognitive architecture must remain responsive as memory
grows.

The practical conclusion: the next architecture branch should prioritize
retrieval/indexing and bounded search. Candidate directions:

- exact fact index for symbolic triples
- approximate nearest-neighbor index for fuzzy semantic lookup
- two-stage retrieval: cheap candidate generation, expensive verification
- negative cache for rejected near-misses
- latency budgets inside answer generation, not only outside benchmarks
- memory compaction or hierarchy for old facts

### 5. QA Chain Depth Is Manageable At Tested Depths, But Still Latency-Sensitive

QA depth passed at all completed depths, including depth `250`:

- `qa-depth-250`: `62/62` passed
- p95 latency for depth `250`: about `28,334 ms`
- `latency-slo-qa-depth-*`: completed rows passed

This suggests chain traversal is not the first place to optimize. It is slower
at high depth, but it is more controlled than memory-pressure retrieval. The
retrieval substrate should be fixed first, then QA-depth should be re-tested
under harder distractors and adversarial chains.

### 6. Ablation Tests Are Still Too Weak

Every ablation variant passed:

- `ablation-matrix-full`: `36/36`
- `ablation-matrix-no-abstraction`: `36/36`
- `ablation-matrix-no-associations`: `36/36`
- `ablation-matrix-no-self-model`: `36/36`
- `ablation-matrix-no-soft-projection`: `36/36`
- `ablation-matrix-no-tool-memory`: `36/36`
- `ablation-matrix-no-trace`: `36/36`

That does not prove every component is equally good. It means the ablation task
does not depend strongly enough on those components. A useful ablation should
cause targeted degradation when a relevant component is removed.

The practical conclusion: ablations need component-specific tasks:

- no associations should hurt analogy or fuzzy linking
- no abstraction should hurt regime/generalization tasks
- no trace should hurt explainability or provenance tests
- no tool memory should hurt repeated tool-selection tasks
- no self-model should hurt confidence calibration and budget allocation

### 7. Chess Shows A Real But Modest Learning Signal

The chess self-play run completed cleanly:

- Games: `82381`
- Plies: `10309642`
- Worker errors: `0`
- First interval agreement: about `0.2505`
- Last interval agreement: about `0.2968`
- Best interval agreement: about `0.3622`
- First cumulative agreement: about `0.2505`
- Last cumulative agreement: about `0.2968`

This is a useful learning signal, but not evidence of strong chess ability. The
model moved closer to Stockfish preference over many games, but agreement
remained low. Chess is useful as a long-running adaptation environment because
it generates large amounts of structured feedback, not because the current
agent is already strategically strong.

The practical conclusion: keep chess as a learning-curve benchmark, but use it
to measure representation and adaptation improvements after retrieval and
verification are fixed.

## What The Tests Did Not Prove

The campaign did not prove general intelligence, robust autonomy, or safe
self-improvement. It also did not prove that high-level cognitive modules are
working deeply. Many old tests passed because the task setup is clean and
synthetic.

The campaign did prove that the benchmark harness can expose failures, that the
program survives heavy parallel execution, and that the current bottlenecks are
measurable.

## Main Conclusions

The current architecture is operationally stable enough for research, but not
yet cognitively robust.

The strongest current capabilities are:

- clean synthetic fact storage and recall
- deterministic QA over controlled chains
- simulated autonomy-budget enforcement
- meta-reasoning classification on synthetic distributions
- long-running chess self-play without worker failure

The strongest current weaknesses are:

- near-miss false positives
- permissive pre-training verification
- inability to reject decoys in hard adaptation
- unbounded or poorly bounded retrieval latency
- ablation tests that do not isolate component value

The most important next branch should not be more breadth. It should be a
retrieval and verification branch that makes memory precise, bounded, and
measurably harder to fool.

## Recommended Next Engineering Branch

### Priority 1: Fact Index And Negative Verification

Implement a stricter fact verification path for `(subject, verb, object)` triples.
The system should distinguish:

- exact known true
- exact known false or contradicted
- unknown
- fuzzy similar but not verified

The benchmark target is:

- `adversarial-qa false_positive_rate <= 0.02`
- `hard-adaptation before_false_positive_rate <= 0.01`
- `hard-adaptation near_miss_rejection >= 0.98`

### Priority 2: Bounded Retrieval

Replace broad scans with indexed candidate retrieval and explicit latency
budgets. Correct answers that arrive after minutes should fail by design.

The benchmark target is:

- `latency-slo-memory-pressure` passes at small and medium first
- p95 latency stays under the SLO
- no max-scale job requires manual timeout to finish

### Priority 3: Stronger Ablations

Rewrite ablation tasks so each disabled component has a specific expected
failure mode. Passing every ablation should be treated as a benchmark bug unless
the component is genuinely unused.

The benchmark target is:

- full system passes
- targeted ablations degrade on their target task
- unrelated ablations do not cause broad accidental collapse

### Priority 4: Chess After Memory Fixes

Re-run chess after retrieval and verification improvements. The desired signal
is not only higher Stockfish agreement, but faster improvement per game and
better stability across openings.

The benchmark target is:

- last interval agreement improves beyond the current `0.2968`
- best interval agreement improves beyond `0.3622`
- learning curve rises faster with fewer games

## Decision For The Next Cycle

The next cycle should focus on making the machine harder to fool, not larger.
Specifically:

1. Add exact and indexed fact verification.
2. Add explicit unknown/false/contradicted states.
3. Add latency budgets to retrieval and QA.
4. Re-run `hard-adaptation`, `adversarial-qa`, and `latency-slo` before adding
   more high-level architecture.

If those three suites improve, then broader cognitive work becomes more
meaningful because the lower-level memory substrate will be less noisy and less
expensive.
