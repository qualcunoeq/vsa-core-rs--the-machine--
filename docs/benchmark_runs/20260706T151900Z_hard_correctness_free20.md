# Hard correctness follow-up run 20260706T151900Z_hard_correctness_free20

## Purpose

Use newly freed capacity for harder non-adaptation correctness coverage while long adaptation and chess runs continue.

## Workloads

- 4 qa-depth max runs.
- 4 ablation-matrix max runs.
- 4 memory-pressure medium runs.
- 2 chaos-run medium runs, 60-minute internal duration.

## Runtime

- CPU affinity: cores 48-67.
- Per-job threads: 20.
- Does not stop or replace current jobs.

## Artifacts

- Results: `results/cognition_bench/20260706T151900Z_hard_correctness_free20`
- Logs: `logs/20260706T151900Z_hard_correctness_free20`
- PIDs: `logs/20260706T151900Z_hard_correctness_free20/pids.txt`
