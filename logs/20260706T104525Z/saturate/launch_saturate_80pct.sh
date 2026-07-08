#!/usr/bin/env bash
set -u
RUN_ID=$(cat /tmp/cognition_bench_run_id)
BIN=./target/release/cognition_bench
mkdir -p results/cognition_bench/$RUN_ID/saturate logs/$RUN_ID/saturate
launch() {
  local label="$1"
  local timeout_dur="$2"
  shift 2
  nohup timeout "$timeout_dur" /usr/bin/time -v "$@" > "logs/$RUN_ID/saturate/${label}.log" 2>&1 &
  echo "$label $!" | tee -a "logs/$RUN_ID/saturate/pids.txt"
}
# CPU-heavy current bottleneck cases. These are intentionally independent
# single-core workers to occupy the 80-vCPU instance without changing code.
for seed in $(seq 10000 10031); do
  launch "memory-pressure_medium_seed${seed}" 2h "$BIN" --case memory-pressure --scale medium --seed "$seed" --out "results/cognition_bench/$RUN_ID/saturate/memory-pressure_medium_seed${seed}.jsonl"
done
for seed in $(seq 20000 20023); do
  launch "adaptation_medium_seed${seed}" 1h "$BIN" --case adaptation --scale medium --seed "$seed" --out "results/cognition_bench/$RUN_ID/saturate/adaptation_medium_seed${seed}.jsonl"
done
for seed in $(seq 30000 30007); do
  launch "meta-reasoning_max_seed${seed}" 1h "$BIN" --case meta-reasoning --scale max --seed "$seed" --out "results/cognition_bench/$RUN_ID/saturate/meta-reasoning_max_seed${seed}.jsonl"
done
