#!/usr/bin/env bash
set -u
FILL_ID=${FILL_ID:-$(cat /tmp/cognition_bench_idle_fill_run_id)}
RESULT_DIR="results/cognition_bench/$FILL_ID"
LOG_DIR="logs/$FILL_ID"
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
# Consume idle cores with important unresolved stressors, avoiding extra adaptation beyond the critical run.
for seed in 92000 92001 92002 92003 92004 92005; do
  launch "memory-pressure_large_idle_seed${seed}" 3h --case memory-pressure --scale large --seed "$seed" --threads 80 --out "$RESULT_DIR/memory-pressure_large_idle_seed${seed}.jsonl"
done
for seed in 92100 92101 92102 92103; do
  launch "qa-depth_max_idle_seed${seed}" 2h --case qa-depth --scale max --seed "$seed" --threads 80 --out "$RESULT_DIR/qa-depth_max_idle_seed${seed}.jsonl"
done
for seed in 92200 92201 92202 92203; do
  launch "memory-pressure_medium_idle_seed${seed}" 2h --case memory-pressure --scale medium --seed "$seed" --threads 80 --out "$RESULT_DIR/memory-pressure_medium_idle_seed${seed}.jsonl"
done
for seed in 92300 92301; do
  launch "chaos-run_medium_idle_seed${seed}" 90m --case chaos-run --scale medium --duration-minutes 60 --seed "$seed" --threads 80 --out "$RESULT_DIR/chaos-run_medium_idle_seed${seed}.jsonl"
done
for seed in 92400 92401; do
  launch "meta-reasoning_max_idle_seed${seed}" 90m --case meta-reasoning --scale max --seed "$seed" --threads 80 --out "$RESULT_DIR/meta-reasoning_max_idle_seed${seed}.jsonl"
done
echo "cluster=$FILL_ID launched=$(wc -l < "$PID_FILE") pid_file=$PID_FILE"
