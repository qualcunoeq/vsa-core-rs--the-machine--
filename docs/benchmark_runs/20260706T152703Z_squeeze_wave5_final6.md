# Squeeze wave 5 final filler run 20260706T152703Z_squeeze_wave5_final6

## Purpose

Final 6-job filler wave after the machine reached roughly 91% CPU usage, intended to consume the remaining idle cores.

## Workloads

- 3 memory-pressure medium runs.
- 3 qa-depth max runs.

## Runtime

- No CPU affinity; allow Linux scheduler to fill idle cores.
- Per-job threads: 80.

## Artifacts

- Results: `results/cognition_bench/20260706T152703Z_squeeze_wave5_final6`
- Logs: `logs/20260706T152703Z_squeeze_wave5_final6`
- PIDs: `logs/20260706T152703Z_squeeze_wave5_final6/pids.txt`
