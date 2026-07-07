# Benchmark Analysis

Generated from recovered raw JSONL artifacts.

## Cognition Benchmarks

- Rows: `3440`
- Passed: `3300`
- Failed: `140`
- Parse errors: `0`

| Experiment | Rows | Passed | Failed | Avg Accuracy | P95 Latency ms | Notes |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| ablation-matrix-full | 36 | 36 | 0 | 1.0000 |  | ablation flags recorded for full |
| ablation-matrix-no-abstraction | 36 | 36 | 0 | 1.0000 |  | ablation flags recorded for no-abstraction |
| ablation-matrix-no-associations | 36 | 36 | 0 | 1.0000 |  | ablation flags recorded for no-associations |
| ablation-matrix-no-self-model | 36 | 36 | 0 | 1.0000 |  | ablation flags recorded for no-self-model |
| ablation-matrix-no-soft-projection | 36 | 36 | 0 | 1.0000 |  | ablation flags recorded for no-soft-projection |
| ablation-matrix-no-tool-memory | 36 | 36 | 0 | 1.0000 |  | ablation flags recorded for no-tool-memory |
| ablation-matrix-no-trace | 36 | 36 | 0 | 1.0000 |  | ablation flags recorded for no-trace |
| adaptation | 92 | 92 | 0 | 1.0000 |  | synthetic feedback inserts facts and retests prior questions |
| adversarial-qa | 36 | 0 | 36 |  | 1133458.40 | near-collision subjects/objects plus explicit negative probes |
| autonomy-budget | 350 | 350 | 0 | 1.0000 |  | simulated actions only; no external side effects |
| chaos-run-summary | 103 | 103 | 0 | 1.0000 |  | mixed workload summary |
| hard-adaptation | 56 | 0 | 56 | 1.0000 | 88216.05 | exact pre/post adaptation with near-miss decoys and regression replay |
| latency-slo-memory-pressure | 48 | 0 | 48 | 1.0000 | 4866179.43 | quality gate for retrieval latency growth |
| latency-slo-qa-depth-10 | 48 | 48 | 0 | 1.0000 | 59.80 | quality gate for QA latency growth |
| latency-slo-qa-depth-100 | 8 | 8 | 0 | 1.0000 | 4005.87 | quality gate for QA latency growth |
| latency-slo-qa-depth-25 | 40 | 40 | 0 | 1.0000 | 289.65 | quality gate for QA latency growth |
| latency-slo-qa-depth-5 | 48 | 48 | 0 | 1.0000 | 24.06 | quality gate for QA latency growth |
| latency-slo-qa-depth-50 | 8 | 8 | 0 | 1.0000 | 1047.14 | quality gate for QA latency growth |
| memory-pressure | 537 | 537 | 0 | 1.0000 | 5447111.31 | synthetic fact insertion and sparse recall probes |
| meta-reasoning | 376 | 376 | 0 | 1.0000 | 48136.63 | synthetic confident/uncertain/stuck classification |
| qa-depth-10 | 364 | 364 | 0 | 1.0000 | 68.66 | synthetic causal chain with distractors |
| qa-depth-100 | 143 | 143 | 0 | 1.0000 | 4462.24 | synthetic causal chain with distractors |
| qa-depth-25 | 362 | 362 | 0 | 1.0000 | 317.78 | synthetic causal chain with distractors |
| qa-depth-250 | 62 | 62 | 0 | 1.0000 | 28334.64 | synthetic causal chain with distractors |
| qa-depth-5 | 302 | 302 | 0 | 1.0000 | 26.85 | synthetic causal chain with distractors |
| qa-depth-50 | 143 | 143 | 0 | 1.0000 | 1172.85 | synthetic causal chain with distractors |
| temporal-abstraction | 62 | 62 | 0 |  | 35.00 | predictive coding under noisy regime switch |

### Metric Ranges
| Metric | N | Min | P50 | P95 | Max |
| --- | ---: | ---: | ---: | ---: | ---: |
| accuracy | 3342 | 1.0000 | 1.0000 | 1.0000 | 1.0000 |
| actions_spent | 350 | 23.0000 | 232.0000 | 11272.1000 | 11484.0000 |
| answer_accuracy | 36 | 1.0000 | 1.0000 | 1.0000 | 1.0000 |
| answer_probes | 36 | 32.0000 | 32.0000 | 32.0000 | 32.0000 |
| avg_latency_ms | 2643 | 0.0005 | 254.6603 | 51025.8017 | 55897.1370 |
| before_accuracy | 92 | 0.9500 | 0.9950 | 0.9950 | 0.9950 |
| before_false_positive_rate | 56 | 0.9950 | 0.9950 | 0.9950 | 0.9950 |
| budget_blocks | 350 | 27.0000 | 286.0000 | 13752.2000 | 13882.0000 |
| chain_depth | 1528 | 5.0000 | 25.0000 | 100.0000 | 250.0000 |
| confidence | 1780 | 0.5897 | 0.6374 | 1.0000 | 1.0000 |
| confidence_error | 62 | 0.0319 | 0.0921 | 0.2000 | 0.6322 |
| confident_count | 376 | 25.0000 | 250.0000 | 12500.0000 | 12500.0000 |
| derived_facts | 1528 | 0.0000 | 0.0000 | 0.0000 | 0.0000 |
| exact_accuracy | 36 | 1.0000 | 1.0000 | 1.0000 | 1.0000 |
| external_write_blocks | 350 | 5.0000 | 55.0000 | 2504.6500 | 2582.0000 |
| fact_count | 585 | 1000.0000 | 10000.0000 | 100000.0000 | 100000.0000 |
| false_positive_rate | 36 | 1.0000 | 1.0000 | 1.0000 | 1.0000 |
| memory_items | 1912 | 1.0000 | 500.0000 | 100000.0000 | 100000.0000 |
| near_miss_rejection | 56 | 0.0000 | 0.0000 | 0.0000 | 0.0000 |
| p95_latency_ms | 2643 | 0.0600 | 611.8074 | 5102580.1701 | 5589713.6967 |
| panic_count | 103 | 0.0000 | 0.0000 | 0.0000 | 0.0000 |
| prediction_error | 62 | 0.0938 | 0.0984 | 0.1062 | 0.6935 |
| regression_rate | 148 | 0.0000 | 0.0000 | 0.0000 | 0.0000 |
| risk_blocks | 350 | 25.0000 | 259.0000 | 12508.9500 | 12671.0000 |
| slo_ms | 200 | 50.0000 | 468.7500 | 8000.0000 | 10000.0000 |
| slo_ratio | 200 | 0.3806 | 0.6142 | 56.4578 | 501.9954 |
| stuck_count | 376 | 12.0000 | 125.0000 | 6250.0000 | 6250.0000 |
| trace_coverage | 1872 | 0.0000 | 1.0000 | 1.0000 | 1.0000 |
| uncertain_count | 376 | 13.0000 | 125.0000 | 6250.0000 | 6250.0000 |

## Chess Self-Play

- Rows: `8251`
- Progress rows: `8231`
- Worker errors: `0`
- Run done marker: `True`
- Games: `82381`
- Plies: `10309642`

| Kind | Count |
| --- | ---: |
| progress | 8231 |
| worker_done | 18 |
| run_start | 1 |
| run_done | 1 |

### Learning Signal
- First interval agreement: `0.25046210720887246`
- Last interval agreement: `0.2968253968253968`
- Best interval agreement: `0.3622`
- Mean interval agreement: `0.2966`
- First cumulative agreement: `0.25046210720887246`
- Last cumulative agreement: `0.29681570524865886`
- Best cumulative agreement: `0.2971`

