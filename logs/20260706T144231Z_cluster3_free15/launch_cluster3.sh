#!/usr/bin/env bash
set -u
EXTRA_ID=${EXTRA_ID:-$(cat /tmp/cognition_bench_cluster3_run_id)}
RESULT_DIR="results/cognition_bench/$EXTRA_ID"
LOG_DIR="logs/$EXTRA_ID"
PID_FILE="$LOG_DIR/pids.txt"
mkdir -p "$RESULT_DIR" "$LOG_DIR"
: > "$PID_FILE"
launch() {
  local name="$1"; shift
  local limit="$1"; shift
  echo "launch $name limit=$limit $*"
  nohup timeout "$limit" taskset -c 18-32 ./target/release/cognition_bench "$@" > "$LOG_DIR/$name.log" 2>&1 &
  echo "$! $name" >> "$PID_FILE"
  sleep 0.1
}
# Free-core cluster: 15 jobs pinned to cores 18-32. Focus on variation and stress, not more adaptation oversubscription.
for seed in $(seq 50000 50004); do
  launch "meta-reasoning_max_seed${seed}" 90m --case meta-reasoning --scale max --seed "$seed" --threads 15 --out "$RESULT_DIR/meta-reasoning_max_seed${seed}.jsonl"
done
for seed in $(seq 50100 50104); do
  launch "autonomy-budget_max_seed${seed}" 90m --case autonomy-budget --scale max --seed "$seed" --threads 15 --out "$RESULT_DIR/autonomy-budget_max_seed${seed}.jsonl"
done
for seed in $(seq 50200 50204); do
  launch "temporal-abstraction_max_seed${seed}" 90m --case temporal-abstraction --scale max --seed "$seed" --threads 15 --out "$RESULT_DIR/temporal-abstraction_max_seed${seed}.jsonl"
done
echo "cluster=$EXTRA_ID launched=$(wc -l < "$PID_FILE") pid_file=$PID_FILE"
