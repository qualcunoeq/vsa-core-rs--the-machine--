#!/usr/bin/env bash
set -u
SAT4_ID=${SAT4_ID:-$(cat /tmp/cognition_bench_squeeze4_run_id)}
RESULT_DIR="results/cognition_bench/$SAT4_ID"
LOG_DIR="logs/$SAT4_ID"
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
for seed in 83000 83001 83002 83003 83004 83005 83006 83007; do
  launch "memory-pressure_large_seed${seed}" 3h --case memory-pressure --scale large --seed "$seed" --threads 80 --out "$RESULT_DIR/memory-pressure_large_seed${seed}.jsonl"
done
for seed in 83100 83101 83102 83103; do
  launch "chaos-run_medium_seed${seed}" 90m --case chaos-run --scale medium --duration-minutes 60 --seed "$seed" --threads 80 --out "$RESULT_DIR/chaos-run_medium_seed${seed}.jsonl"
done
for seed in 83200 83201 83202 83203; do
  launch "qa-depth_max_seed${seed}" 2h --case qa-depth --scale max --seed "$seed" --threads 80 --out "$RESULT_DIR/qa-depth_max_seed${seed}.jsonl"
done
for seed in 83300 83301 83302 83303; do
  launch "memory-pressure_medium_seed${seed}" 2h --case memory-pressure --scale medium --seed "$seed" --threads 80 --out "$RESULT_DIR/memory-pressure_medium_seed${seed}.jsonl"
done
echo "cluster=$SAT4_ID launched=$(wc -l < "$PID_FILE") pid_file=$PID_FILE"
