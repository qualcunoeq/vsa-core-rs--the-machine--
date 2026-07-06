#!/usr/bin/env bash
set -u
SAT3_ID=${SAT3_ID:-$(cat /tmp/cognition_bench_squeeze3_run_id)}
RESULT_DIR="results/cognition_bench/$SAT3_ID"
LOG_DIR="logs/$SAT3_ID"
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
for seed in 82000 82001 82002 82003; do
  launch "memory-pressure_medium_seed${seed}" 2h --case memory-pressure --scale medium --seed "$seed" --threads 80 --out "$RESULT_DIR/memory-pressure_medium_seed${seed}.jsonl"
done
for seed in 82100 82101 82102 82103; do
  launch "qa-depth_max_seed${seed}" 2h --case qa-depth --scale max --seed "$seed" --threads 80 --out "$RESULT_DIR/qa-depth_max_seed${seed}.jsonl"
done
for seed in 82200 82201 82202; do
  launch "meta-reasoning_max_seed${seed}" 90m --case meta-reasoning --scale max --seed "$seed" --threads 80 --out "$RESULT_DIR/meta-reasoning_max_seed${seed}.jsonl"
done
for seed in 82300 82301 82302; do
  launch "autonomy-budget_max_seed${seed}" 90m --case autonomy-budget --scale max --seed "$seed" --threads 80 --out "$RESULT_DIR/autonomy-budget_max_seed${seed}.jsonl"
done
echo "cluster=$SAT3_ID launched=$(wc -l < "$PID_FILE") pid_file=$PID_FILE"
