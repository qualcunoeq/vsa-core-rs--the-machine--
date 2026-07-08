# Adaptation diagnosis run 20260706T145016Z_adaptation_diag_freecores

## Purpose

Diagnose why adaptation medium/large workloads are long-running or failing to emit JSONL while preserving the active cluster2 and chess runs.

## Runtime Plan

- CPU affinity: cores 33-47.
- Small controls: 3 jobs, scale small, timeout 20m, threads 8.
- Medium profile: 6 jobs, scale medium, timeout 90m, threads 12.
- Large sentinels: 2 jobs, scale large, timeout 3h, threads 12.
- Monitor cadence: every 10 minutes into `logs/20260706T145016Z_adaptation_diag_freecores/monitor.jsonl`.

## Artifacts

- Results: `results/cognition_bench/20260706T145016Z_adaptation_diag_freecores`
- Logs: `logs/20260706T145016Z_adaptation_diag_freecores`
- PIDs: `logs/20260706T145016Z_adaptation_diag_freecores/pids.txt`
- Monitor: `logs/20260706T145016Z_adaptation_diag_freecores/monitor.jsonl`
