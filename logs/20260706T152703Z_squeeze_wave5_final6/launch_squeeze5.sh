#!/usr/bin/env bash
set -u
SAT5_ID=${SAT5_ID:-$(cat /tmp/cognition_bench_squeeze5_run_id)}
RESULT_DIR="results/cognition_bench/$SAT5_ID"
LOG_DIR="logs/$SAT5_ID"
PID_FILE="$LOG_DIR/pids.txt"
mkdir -p "$RESULT_DIR" "$LOG_DIR"
: > "$PID_FILE"
launch() {
  local name="$1"; shift
  local limit="$1"; shift
  echo "launch $name limit=$limit $*"
  nohup timeout "$limit" ./target/release/cognition_bench "$@" > "$LOG_DIR/$name.log" 2>&1 &
  echo "$! $name" >> "$PID_FILE"
  sleep 0.04
}
for seed in 84000 84001 84002; do
  launch "memory-pressure_medium_seed${seed}" 2h --case memory-pressure --scale medium --seed "$seed" --threads 80 --out "$RESULT_DIR/memory-pressure_medium_seed${seed}.jsonl"
done
for seed in 84100 84101 84102; do
  launch "qa-depth_max_seed${seed}" 2h --case qa-depth --scale max --seed "$seed" --threads 80 --out "$RESULT_DIR/qa-depth_max_seed${seed}.jsonl"
done
echo "cluster=$SAT5_ID launched=$(wc -l < "$PID_FILE") pid_file=$PID_FILE"
