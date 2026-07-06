#!/usr/bin/env bash
set -u
SAT2_ID=${SAT2_ID:-$(cat /tmp/cognition_bench_squeeze2_run_id)}
RESULT_DIR="results/cognition_bench/$SAT2_ID"
LOG_DIR="logs/$SAT2_ID"
PID_FILE="$LOG_DIR/pids.txt"
mkdir -p "$RESULT_DIR" "$LOG_DIR"
: > "$PID_FILE"
launch() {
  local name="$1"; shift
  local limit="$1"; shift
  echo "launch $name limit=$limit $*"
  nohup timeout "$limit" taskset -c 32-79 ./target/release/cognition_bench "$@" > "$LOG_DIR/$name.log" 2>&1 &
  echo "$! $name" >> "$PID_FILE"
  sleep 0.08
}
for seed in 81000 81001 81002 81003 81004 81005; do
  launch "temporal-abstraction_max_seed${seed}" 90m --case temporal-abstraction --scale max --seed "$seed" --threads 48 --out "$RESULT_DIR/temporal-abstraction_max_seed${seed}.jsonl"
done
for seed in 81100 81101 81102 81103 81104 81105; do
  launch "autonomy-budget_max_seed${seed}" 90m --case autonomy-budget --scale max --seed "$seed" --threads 48 --out "$RESULT_DIR/autonomy-budget_max_seed${seed}.jsonl"
done
for seed in 81200 81201 81202 81203 81204 81205; do
  launch "meta-reasoning_max_seed${seed}" 90m --case meta-reasoning --scale max --seed "$seed" --threads 48 --out "$RESULT_DIR/meta-reasoning_max_seed${seed}.jsonl"
done
for seed in 81300 81301 81302 81303; do
  launch "qa-depth_max_seed${seed}" 2h --case qa-depth --scale max --seed "$seed" --threads 48 --out "$RESULT_DIR/qa-depth_max_seed${seed}.jsonl"
done
for seed in 81400 81401; do
  launch "memory-pressure_medium_seed${seed}" 2h --case memory-pressure --scale medium --seed "$seed" --threads 48 --out "$RESULT_DIR/memory-pressure_medium_seed${seed}.jsonl"
done
echo "cluster=$SAT2_ID launched=$(wc -l < "$PID_FILE") pid_file=$PID_FILE"
