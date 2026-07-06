# Memory scaling ladder run 20260706T163951Z_memory_scaling_ladder

## Purpose

Quantify memory retrieval latency across medium, large, and max scales after current pressure drops.

## Workloads

- 4 memory-pressure medium jobs, timeout 2h.
- 4 memory-pressure large jobs, timeout 4h.
- 2 memory-pressure max jobs, timeout 6h.

## Runtime

- Starts when at least 22 tracked cores are free.
- Per-job threads: 80.

## Artifacts

- Results: `results/cognition_bench/20260706T163951Z_memory_scaling_ladder`
- Logs: `logs/20260706T163951Z_memory_scaling_ladder`
- PIDs: `logs/20260706T163951Z_memory_scaling_ladder/pids.txt`
