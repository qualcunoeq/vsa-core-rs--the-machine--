# Manual sustained run 20260706T170645Z_manual_sustained

## Purpose

Second manual run to consume remaining idle cores with sustained memory-pressure and chaos workloads after fast replay tasks completed quickly.

## Workloads

- 6 memory-pressure large jobs.
- 4 memory-pressure medium jobs.
- 4 chaos-run medium jobs with 60-minute internal duration.

## Runtime

- Per-job threads: 80.
- No CPU affinity; allow scheduler to fill idle cores.
- Timeouts: 4h large memory, 2h medium memory, 90m chaos.

## Artifacts

- Results: `results/cognition_bench/20260706T170645Z_manual_sustained`
- Logs: `logs/20260706T170645Z_manual_sustained`
- PIDs: `logs/20260706T170645Z_manual_sustained/pids.txt`
