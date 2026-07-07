# Benchmark Analysis

Generated from recovered raw JSONL artifacts.

## Cognition Benchmarks

- Rows: `2853`
- Passed: `2850`
- Failed: `3`
- Parse errors: `0`

| Experiment | Rows | Passed | Failed | Avg Accuracy | P95 Latency ms | Notes |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| ablation-matrix-full | 20 | 20 | 0 | 1.0000 |  | ablation flags recorded for full |
| ablation-matrix-no-abstraction | 20 | 20 | 0 | 1.0000 |  | ablation flags recorded for no-abstraction |
| ablation-matrix-no-associations | 20 | 20 | 0 | 1.0000 |  | ablation flags recorded for no-associations |
| ablation-matrix-no-self-model | 20 | 20 | 0 | 1.0000 |  | ablation flags recorded for no-self-model |
| ablation-matrix-no-soft-projection | 20 | 20 | 0 | 1.0000 |  | ablation flags recorded for no-soft-projection |
| ablation-matrix-no-tool-memory | 20 | 20 | 0 | 1.0000 |  | ablation flags recorded for no-tool-memory |
| ablation-matrix-no-trace | 20 | 20 | 0 | 1.0000 |  | ablation flags recorded for no-trace |
| adaptation | 92 | 92 | 0 | 1.0000 |  | synthetic feedback inserts facts and retests prior questions |
| adversarial-qa | 1 | 0 | 1 |  | 3747.94 | near-collision subjects/objects plus explicit negative probes |
| autonomy-budget | 330 | 330 | 0 | 1.0000 |  | simulated actions only; no external side effects |
| chaos-run-summary | 103 | 103 | 0 | 1.0000 |  | mixed workload summary |
| hard-adaptation | 1 | 0 | 1 | 1.0000 | 186.16 | exact pre/post adaptation with near-miss decoys and regression replay |
| latency-slo-memory-pressure | 1 | 0 | 1 | 1.0000 | 10566.92 | quality gate for retrieval latency growth |
| latency-slo-qa-depth-10 | 1 | 1 | 0 | 1.0000 | 26.42 | quality gate for QA latency growth |
| latency-slo-qa-depth-5 | 1 | 1 | 0 | 1.0000 | 10.39 | quality gate for QA latency growth |
| memory-pressure | 529 | 529 | 0 | 1.0000 | 5449511.06 | synthetic fact insertion and sparse recall probes |
| meta-reasoning | 344 | 344 | 0 | 1.0000 | 48055.81 | synthetic confident/uncertain/stuck classification |
| qa-depth-10 | 344 | 344 | 0 | 1.0000 | 73.29 | synthetic causal chain with distractors |
| qa-depth-100 | 123 | 123 | 0 | 1.0000 | 4488.15 | synthetic causal chain with distractors |
| qa-depth-25 | 342 | 342 | 0 | 1.0000 | 317.78 | synthetic causal chain with distractors |
| qa-depth-250 | 50 | 50 | 0 | 1.0000 | 28718.15 | synthetic causal chain with distractors |
| qa-depth-5 | 294 | 294 | 0 | 1.0000 | 26.88 | synthetic causal chain with distractors |
| qa-depth-50 | 123 | 123 | 0 | 1.0000 | 1192.45 | synthetic causal chain with distractors |
| temporal-abstraction | 34 | 34 | 0 |  | 35.94 | predictive coding under noisy regime switch |

### Metric Ranges
| Metric | N | Min | P50 | P95 | Max |
| --- | ---: | ---: | ---: | ---: | ---: |
| accuracy | 2818 | 1.0000 | 1.0000 | 1.0000 | 1.0000 |
| actions_spent | 330 | 23.0000 | 231.0000 | 11215.9500 | 11345.0000 |
| answer_accuracy | 1 | 1.0000 | 1.0000 | 1.0000 | 1.0000 |
| answer_probes | 1 | 16.0000 | 16.0000 | 16.0000 | 16.0000 |
| avg_latency_ms | 2188 | 0.0005 | 213.5667 | 51797.7330 | 55897.1370 |
| before_accuracy | 92 | 0.9500 | 0.9950 | 0.9950 | 0.9950 |
| before_false_positive_rate | 1 | 0.9500 | 0.9500 | 0.9500 | 0.9500 |
| budget_blocks | 330 | 27.0000 | 284.0000 | 13731.2000 | 13882.0000 |
| chain_depth | 1278 | 5.0000 | 10.0000 | 100.0000 | 250.0000 |
| confidence | 1418 | 0.5897 | 0.6374 | 1.0000 | 1.0000 |
| confidence_error | 34 | 0.0319 | 0.0908 | 0.2133 | 0.6322 |
| confident_count | 344 | 25.0000 | 250.0000 | 12500.0000 | 12500.0000 |
| derived_facts | 1278 | 0.0000 | 0.0000 | 0.0000 | 0.0000 |
| exact_accuracy | 1 | 1.0000 | 1.0000 | 1.0000 | 1.0000 |
| external_write_blocks | 330 | 5.0000 | 54.0000 | 2481.1000 | 2582.0000 |
| fact_count | 530 | 1000.0000 | 10000.0000 | 100000.0000 | 100000.0000 |
| false_positive_rate | 1 | 1.0000 | 1.0000 | 1.0000 | 1.0000 |
| memory_items | 1575 | 1.0000 | 5000.0000 | 100000.0000 | 100000.0000 |
| near_miss_rejection | 1 | 0.0000 | 0.0000 | 0.0000 | 0.0000 |
| p95_latency_ms | 2188 | 0.0600 | 590.2699 | 5179773.2965 | 5589713.6967 |
| panic_count | 103 | 0.0000 | 0.0000 | 0.0000 | 0.0000 |
| prediction_error | 34 | 0.0950 | 0.0992 | 0.1069 | 0.6935 |
| regression_rate | 93 | 0.0000 | 0.0000 | 0.0000 | 0.0000 |
| risk_blocks | 330 | 25.0000 | 257.5000 | 12465.5000 | 12671.0000 |
| slo_ms | 3 | 50.0000 | 75.0000 | 5407.5000 | 6000.0000 |
| slo_ratio | 3 | 0.2079 | 0.3522 | 1.6203 | 1.7612 |
| stuck_count | 344 | 12.0000 | 125.0000 | 6250.0000 | 6250.0000 |
| trace_coverage | 1510 | 0.0000 | 1.0000 | 1.0000 | 1.0000 |
| uncertain_count | 344 | 13.0000 | 125.0000 | 6250.0000 | 6250.0000 |

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

