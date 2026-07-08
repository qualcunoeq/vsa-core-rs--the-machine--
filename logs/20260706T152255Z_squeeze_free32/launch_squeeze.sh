#!/usr/bin/env bash
set -u
SAT_ID=${SAT_ID:-$(cat /tmp/cognition_bench_squeeze_run_id)}
RESULT_DIR="results/cognition_bench/$SAT_ID"
LOG_DIR="logs/$SAT_ID"
PID_FILE="$LOG_DIR/pids.txt"
mkdir -p "$RESULT_DIR" "$LOG_DIR"
: > "$PID_FILE"
launch() {
  local name="$1"; shift
  local limit="$1"; shift
  echo "launch $name limit=$limit $*"
  nohup timeout "$limit" taskset -c 0-31 ./target/release/cognition_bench "$@" > "$LOG_DIR/$name.log" 2>&1 &
  echo "$! $name" >> "$PID_FILE"
  sleep 0.1
}
# Use remaining capacity aggressively, but avoid new adaptation load because it is already under diagnosis.
for seed in 80000 80001 80002 80003 80004 80005 80006 80007; do
  launch "memory-pressure_medium_seed${seed}" 2h --case memory-pressure --scale medium --seed "$seed" --threads 32 --out "$RESULT_DIR/memory-pressure_medium_seed${seed}.jsonl"
done
for seed in 80100 80101 80102 80103; do
  launch "memory-pressure_large_seed${seed}" 3h --case memory-pressure --scale large --seed "$seed" --threads 32 --out "$RESULT_DIR/memory-pressure_large_seed${seed}.jsonl"
done
for seed in 80200 80201 80202 80203; do
  launch "qa-depth_max_seed${seed}" 2h --case qa-depth --scale max --seed "$seed" --threads 32 --out "$RESULT_DIR/qa-depth_max_seed${seed}.jsonl"
done
for seed in 80300 80301 80302 80303; do
  launch "meta-reasoning_max_seed${seed}" 90m --case meta-reasoning --scale max --seed "$seed" --threads 32 --out "$RESULT_DIR/meta-reasoning_max_seed${seed}.jsonl"
done
for seed in 80400 80401; do
  launch "chaos-run_medium_seed${seed}" 90m --case chaos-run --scale medium --duration-minutes 60 --seed "$seed" --threads 32 --out "$RESULT_DIR/chaos-run_medium_seed${seed}.jsonl"
done
echo "cluster=$SAT_ID launched=$(wc -l < "$PID_FILE") pid_file=$PID_FILE"
