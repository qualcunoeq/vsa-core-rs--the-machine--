# What We Have Learned About The Machine

As of: 2026-07-06T16:22:26Z

## Executive Summary

The current evidence says The Machine is stable and broad, but not yet deeply discriminating. Across the parsed cognition benchmark artifacts available at this snapshot, every parsed result row passed. That is useful evidence of execution stability, but it also means the benchmark suite is not yet hard enough to separate real architectural capability from synthetic-task fit.

The strongest technical signal is not correctness failure; it is scaling pressure. Deep QA and memory-pressure latency grow sharply. Adaptation large/max remains the main unresolved completion/stall question. The Stockfish self-play run shows a weak positive learning curve, but it is not conclusive until the run finishes and a postrun evaluation is generated.

## Benchmark Campaign Overview

- Cognition result rows parsed: `801`
- Cognition rows passed: `801`
- Cognition rows failed: `0`
- Distinct experiment labels observed: `19`

| Run | JSONL files | Rows | Passed | Failed |
| --- | ---: | ---: | ---: | ---: |
| `20260706T104525Z` | 110 | 271 | 271 | 0 |
| `20260706T141200Z_cluster2` | 86 | 118 | 118 | 0 |
| `20260706T144231Z_cluster3_free15` | 15 | 15 | 15 | 0 |
| `20260706T145016Z_adaptation_diag_freecores` | 9 | 9 | 9 | 0 |
| `20260706T151900Z_hard_correctness_free20` | 14 | 156 | 156 | 0 |
| `20260706T152255Z_squeeze_free32` | 16 | 32 | 32 | 0 |
| `20260706T152411Z_squeeze_wave2_free24` | 24 | 40 | 40 | 0 |
| `20260706T152510Z_squeeze_wave3_free14` | 14 | 30 | 30 | 0 |
| `20260706T152610Z_squeeze_wave4_fill20` | 8 | 24 | 24 | 0 |
| `20260706T152703Z_squeeze_wave5_final6` | 6 | 18 | 18 | 0 |
| `20260706T155344Z_critical_signal` | 4 | 20 | 20 | 0 |
| `20260706T155717Z_remaining_capacity` | 18 | 42 | 42 | 0 |
| `20260706T160400Z_idle_fill` | 10 | 26 | 26 | 0 |

Major exercised families include QA depth, memory pressure, adaptation, ablation matrix, temporal abstraction, meta-reasoning, autonomy budget, chaos durability, and Stockfish-guided chess self-play.

## What The Machine Does Well

- It is operationally stable under heavy concurrent benchmark pressure. We launched many overlapping runs and still parsed valid JSONL outputs without observed parse failures in the summarized artifacts.
- It handles the current synthetic cognition tasks with perfect parsed pass rate. This includes deep QA chains, memory-pressure tasks, temporal abstraction, meta-reasoning, autonomy budget, and adaptation small/medium outputs.
- Trace and confidence metrics remain clean in the benchmark paths that report them. The explicit `no-trace` ablation correctly drops trace coverage while still passing the simple task.
- It remains memory-light relative to the 80 GiB machine. The bottleneck is CPU time and algorithmic scaling, not RAM exhaustion.

## Where The Machine Struggles

- Deep QA latency grows hard with chain depth. Current `qa-depth-10` avg-latency range is `45.7 / 1534 / 5178 ms`, while `qa-depth-250` is `1.737e+04 / 2.417e+04 / 2.947e+04 ms`.
- Memory-pressure remains the clearest retrieval bottleneck. Observed memory-pressure avg-latency range is `3578 / 6931 / 5.17e+04 ms`, across fact-count range `1e+04 / 1.514e+04 / 1e+05`.
- Adaptation large/max is still unresolved. Small and medium adaptation outputs can pass, but several large/max adaptation jobs have historically remained pending or produced no JSONL within their windows.
- Ablation tests are too weak. Most feature-removal variants still pass perfectly, which means the benchmark does not yet require those architectural features in a discriminating way.
- Pass/fail alone is now low-value. We need latency curves, completion behavior, and adversarial failures to learn more.

## Chess Self-Play Findings

- Chess run: `20260706T143421Z_chess_stockfish_18c_4h`
- JSONL rows: `3602`
- Progress rows: `3601`
- Workers reporting: `18`
- Latest games: `36010`
- Latest plies: `4570781`
- First interval Stockfish agreement: `0.2478`
- Latest interval Stockfish agreement: `0.3000`
- Latest cumulative agreement: `0.2830`
- First interval loss: `0.7181`
- Latest interval loss: `0.7371`
- Run done marker present: `False`

The chess encoder is sparse and domain-specific: piece occupancy planes, side/castling/en-passant/phase bits, candidate move bits, and resulting-board bits. The online learner updates by raising Stockfish move features and lowering the selected student move features. The agreement increase is promising, but not proof of strong chess reasoning; it may partly reflect shallow move priors and should be judged after postrun evaluation.

## Engineering Lessons

- Add progress instrumentation inside adaptation. A job that runs for hours without JSONL is hard to distinguish from a productive long computation.
- Improve memory retrieval/indexing before adding more broad benchmarks. The accuracy is already high; latency is the bottleneck.
- Make ablation tests adversarial. If removing associations, abstraction, self-model, or tool memory still passes, the test is not proving those systems matter.
- Separate saturated and low-contention runs. Saturation is good for stress testing, but clean latency baselines need isolated replays.
- Automate final summaries. Manual parsing works, but repeated benchmark waves need standard summarizer output for pass/fail, latency, completion, timeout, and missing-JSONL status.

## Next Research Priorities

1. Run `adaptation_isolation` to determine whether large/max adaptation completes under cleaner conditions.
2. Run `memory_scaling_ladder` to measure medium/large/max retrieval latency with fewer confounders.
3. Run `low_contention_replay` after heavy jobs finish to compare against saturated latency.
4. Run `chess_postrun_eval` after the Stockfish run writes `run_done`.
5. Build harder adversarial benchmarks where ablations should fail when the removed capability matters.

## Raw Artifacts

- Cognition results live under `results/cognition_bench/<RUN_ID>/`.
- Chess results live under `results/chess_stockfish/<RUN_ID>/`.
- Run documentation lives under `docs/benchmark_runs/`.
- The queued next-test plan is `docs/benchmark_runs/queued_next_tests.md`.
