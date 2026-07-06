# Manual priority run 20260706T170551Z_manual_priority

## Purpose

Manual immediate run requested after observing idle cores. This complements the active memory ladder with adaptation probes, chaos durability, ablation checks, and deep QA replay.

## Workloads

- 4 adaptation medium jobs.
- 2 adaptation large jobs.
- 4 chaos-run medium jobs with 60-minute internal duration.
- 4 ablation-matrix max jobs.
- 4 qa-depth max jobs.

## Runtime

- Per-job threads: 80.
- No CPU affinity; allow scheduler to fill idle cores.
- Timeouts: 90m medium adaptation/chaos/ablation, 2h QA, 4h large adaptation.

## Artifacts

- Results: `results/cognition_bench/20260706T170551Z_manual_priority`
- Logs: `logs/20260706T170551Z_manual_priority`
- PIDs: `logs/20260706T170551Z_manual_priority/pids.txt`
