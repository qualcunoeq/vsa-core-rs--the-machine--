# Critical signal run 20260706T155344Z_critical_signal

## Purpose

Focus new capacity on the important unresolved signals: adaptation scaling/stalls, large memory-pressure latency, deep QA latency, and chaos durability under live load.

## Workloads

- 4 adaptation medium critical runs.
- 2 adaptation large critical runs.
- 6 memory-pressure large critical runs.
- 4 qa-depth max critical runs.
- 4 chaos-run medium critical runs with 60-minute internal duration.

## Runtime

- Per-job threads: 80.
- Timeouts: 90m for medium adaptation and chaos, 2h for QA, 3h for large adaptation and large memory-pressure.
- No CPU affinity; allow scheduler to use currently idle cores.

## Artifacts

- Results: `results/cognition_bench/20260706T155344Z_critical_signal`
- Logs: `logs/20260706T155344Z_critical_signal`
- PIDs: `logs/20260706T155344Z_critical_signal/pids.txt`
