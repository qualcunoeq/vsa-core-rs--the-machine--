#!/usr/bin/env bash
set -u
RUN_ID=$(cat /tmp/cognition_bench_run_id)
mkdir -p results/cognition_bench/$RUN_ID logs/$RUN_ID
launch() {
  local label="$1"
  local timeout_dur="$2"
  shift 2
  nohup timeout "$timeout_dur" /usr/bin/time -v "$@" > "logs/$RUN_ID/${label}.log" 2>&1 &
  echo "$label $!" | tee -a "logs/$RUN_ID/pids_full.txt"
}
BIN=./target/release/cognition_bench
for seed in 99 2026 9001; do
  for case in qa-depth ablation-matrix temporal-abstraction meta-reasoning autonomy-budget; do
    launch "${case}_max_seed${seed}" 3h "$BIN" --case "$case" --scale max --seed "$seed" --out "results/cognition_bench/$RUN_ID/${case}_max_seed${seed}.jsonl"
  done
done
for seed in 1 7 42 1337; do
  launch "memory-pressure_medium_seed${seed}" 2h "$BIN" --case memory-pressure --scale medium --seed "$seed" --out "results/cognition_bench/$RUN_ID/memory-pressure_medium_seed${seed}.jsonl"
done
for seed in 42 1337; do
  launch "memory-pressure_large_seed${seed}" 3h "$BIN" --case memory-pressure --scale large --seed "$seed" --out "results/cognition_bench/$RUN_ID/memory-pressure_large_seed${seed}.jsonl"
done
for seed in 1 7 42 1337; do
  launch "adaptation_medium_seed${seed}" 1h "$BIN" --case adaptation --scale medium --seed "$seed" --out "results/cognition_bench/$RUN_ID/adaptation_medium_seed${seed}.jsonl"
done
for seed in 42 1337; do
  launch "adaptation_large_seed${seed}" 2h "$BIN" --case adaptation --scale large --seed "$seed" --out "results/cognition_bench/$RUN_ID/adaptation_large_seed${seed}.jsonl"
done
launch "chaos-run_medium_seed42_90m" 2h "$BIN" --case chaos-run --scale medium --seed 42 --duration-minutes 90 --out "results/cognition_bench/$RUN_ID/chaos-run_medium_seed42_90m.jsonl"
