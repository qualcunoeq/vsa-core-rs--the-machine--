#!/usr/bin/env bash
set -u
MANUAL2_ID=${MANUAL2_ID:-$(cat /tmp/cognition_bench_manual_sustained_run_id)}
RESULT_DIR="results/cognition_bench/$MANUAL2_ID"
LOG_DIR="logs/$MANUAL2_ID"
PID_FILE="$LOG_DIR/pids.txt"
mkdir -p "$RESULT_DIR" "$LOG_DIR"
: > "$PID_FILE"
launch() {
  local name="$1"; shift
  local limit="$1"; shift
  echo "launch $name limit=$limit $*"
  nohup timeout "$limit" ./target/release/cognition_bench "$@" > "$LOG_DIR/$name.log" 2>&1 &
  echo "$! $name" >> "$PID_FILE"
  sleep 0.05
}
for seed in 97000 97001 97002 97003 97004 97005; do
  launch "memory-pressure_large_manual2_seed${seed}" 4h --case memory-pressure --scale large --seed "$seed" --threads 80 --out "$RESULT_DIR/memory-pressure_large_manual2_seed${seed}.jsonl"
done
for seed in 97100 97101 97102 97103; do
  launch "memory-pressure_medium_manual2_seed${seed}" 2h --case memory-pressure --scale medium --seed "$seed" --threads 80 --out "$RESULT_DIR/memory-pressure_medium_manual2_seed${seed}.jsonl"
done
for seed in 97200 97201 97202 97203; do
  launch "chaos-run_medium_manual2_seed${seed}" 90m --case chaos-run --scale medium --duration-minutes 60 --seed "$seed" --threads 80 --out "$RESULT_DIR/chaos-run_medium_manual2_seed${seed}.jsonl"
done
echo "cluster=$MANUAL2_ID launched=$(wc -l < "$PID_FILE") pid_file=$PID_FILE"
