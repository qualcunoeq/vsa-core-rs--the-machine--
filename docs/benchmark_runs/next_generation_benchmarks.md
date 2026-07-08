# Next-Generation Benchmarks

The first rented-server campaign proved operational stability, but the pass rate
was too high to measure intelligence. The next benchmark branch focuses on
failure-seeking tests that should expose weak retrieval, shallow adaptation, and
non-discriminative ablations.

## New Benchmark Cases

- `hard-adaptation`: exact pre/post learning with near-miss decoys and replayed
  regression checks. This replaces confidence-only before/after scoring with
  exact fact verification.
- `adversarial-qa`: near-collision subjects and objects plus explicit negative
  probes. This is intended to catch false positives and shallow lexical lookup.
- `latency-slo`: wraps QA depth and memory pressure in concrete latency quality
  gates. Correct-but-too-slow behavior now fails.

## What These Fix

- Adaptation now starts from a low baseline and must show real post-feedback
  recall.
- Memory and QA tests now record whether latency is acceptable, not only whether
  answers are eventually correct.
- Adversarial QA gives us a false-positive rate, which was missing from the
  first campaign.
- The result analyzer converts raw JSONL into trend reports so we do not depend
  on manual checkpoint summaries.

## Recommended Next Run

Run these locally or on a rented instance:

```bash
cargo test
cargo build --release --bin cognition_bench
./target/release/cognition_bench --case hard-adaptation --scale medium --seed 1 --out results/cognition_bench/next_gen/hard_adaptation_medium_seed1.jsonl
./target/release/cognition_bench --case adversarial-qa --scale medium --seed 1 --out results/cognition_bench/next_gen/adversarial_qa_medium_seed1.jsonl
./target/release/cognition_bench --case latency-slo --scale medium --seed 1 --out results/cognition_bench/next_gen/latency_slo_medium_seed1.jsonl
python3 experiments/analyze_benchmark_results.py --results results --out docs/benchmark_runs/analysis_latest.md
```

For a larger machine, repeat with `--scale large` and multiple seeds. Treat
failures as useful signal, not regressions, unless they are crashes or parse
errors.

## Interpreting Results

- If `hard-adaptation` fails on `before_false_positive_rate`, the QA memory is
  hallucinating facts before feedback.
- If it fails on `near_miss_rejection`, exact recall is too permissive around
  similar concepts.
- If `adversarial-qa` fails on `answer_accuracy`, the natural-language question
  path is weaker than exact fact verification.
- If `latency-slo` fails, prioritize retrieval/indexing or hierarchical caching
  before adding broader cognitive features.
