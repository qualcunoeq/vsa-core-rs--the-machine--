# Remaining capacity run 20260706T155717Z_remaining_capacity

## Purpose

Use remaining CPU while the critical branch runs, focusing on broad correctness and safety signals rather than adding more adaptation load.

## Workloads

- 4 ablation-matrix max runs.
- 4 temporal-abstraction max runs.
- 4 autonomy-budget max runs.
- 4 meta-reasoning max runs.
- 2 memory-pressure medium runs.

## Runtime

- Per-job threads: 80.
- No CPU affinity; allow scheduler to fill idle cores.
- Timeouts: 90m for fast cognition checks, 2h for memory-pressure.

## Artifacts

- Results: `results/cognition_bench/20260706T155717Z_remaining_capacity`
- Logs: `logs/20260706T155717Z_remaining_capacity`
- PIDs: `logs/20260706T155717Z_remaining_capacity/pids.txt`
