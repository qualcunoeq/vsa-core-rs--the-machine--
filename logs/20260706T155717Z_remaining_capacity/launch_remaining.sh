#!/usr/bin/env bash
set -u
REM_ID=${REM_ID:-$(cat /tmp/cognition_bench_remaining_run_id)}
RESULT_DIR="results/cognition_bench/$REM_ID"
LOG_DIR="logs/$REM_ID"
PID_FILE="$LOG_DIR/pids.txt"
mkdir -p "$RESULT_DIR" "$LOG_DIR"
: > "$PID_FILE"
launch() {
  local name="$1"; shift
  local limit="$1"; shift
  echo "launch $name limit=$limit $*"
  nohup timeout "$limit" ./target/release/cognition_bench "$@" > "$LOG_DIR/$name.log" 2>&1 &
  echo "$! $name" >> "$PID_FILE"
  sleep 0.06
}
# Remaining capacity branch: useful broad coverage while critical adaptation/memory jobs run.
for seed in 91000 91001 91002 91003; do
  launch "ablation-matrix_max_remaining_seed${seed}" 90m --case ablation-matrix --scale max --seed "$seed" --threads 80 --out "$RESULT_DIR/ablation-matrix_max_remaining_seed${seed}.jsonl"
done
for seed in 91100 91101 91102 91103; do
  launch "temporal-abstraction_max_remaining_seed${seed}" 90m --case temporal-abstraction --scale max --seed "$seed" --threads 80 --out "$RESULT_DIR/temporal-abstraction_max_remaining_seed${seed}.jsonl"
done
for seed in 91200 91201 91202 91203; do
  launch "autonomy-budget_max_remaining_seed${seed}" 90m --case autonomy-budget --scale max --seed "$seed" --threads 80 --out "$RESULT_DIR/autonomy-budget_max_remaining_seed${seed}.jsonl"
done
for seed in 91300 91301 91302 91303; do
  launch "meta-reasoning_max_remaining_seed${seed}" 90m --case meta-reasoning --scale max --seed "$seed" --threads 80 --out "$RESULT_DIR/meta-reasoning_max_remaining_seed${seed}.jsonl"
done
for seed in 91400 91401; do
  launch "memory-pressure_medium_remaining_seed${seed}" 2h --case memory-pressure --scale medium --seed "$seed" --threads 80 --out "$RESULT_DIR/memory-pressure_medium_remaining_seed${seed}.jsonl"
done
echo "cluster=$REM_ID launched=$(wc -l < "$PID_FILE") pid_file=$PID_FILE"
