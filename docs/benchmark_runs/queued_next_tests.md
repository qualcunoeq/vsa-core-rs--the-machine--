# Queued next tests

## Purpose

Automatically start the next meaningful benchmark clusters when current heavy jobs finish and capacity becomes useful again.

## Queue Order

1. `memory_scaling_ladder`: medium/large/max memory-pressure latency curve, allowed to start when enough idle capacity exists.
2. `adaptation_isolation`: medium/large/max adaptation in cleaner conditions, still waits for lower contention.
3. `low_contention_replay`: broad cognition replay after heavy jobs finish.
4. `chess_postrun_eval`: parse chess learning curve after the Stockfish run writes `run_done`.

## Behavior

- Polls every 5 minutes.
- Starts adaptation isolation when active `cognition_bench` usage is below 20 cores.
- Starts memory scaling when at least 22 tracked cores are free, even if adaptation isolation is still waiting for lower contention.
- Starts low-contention replay after adaptation and memory queued clusters finish.
- Writes queue monitor rows to `logs/queued_next_tests/queue_monitor.jsonl`.
