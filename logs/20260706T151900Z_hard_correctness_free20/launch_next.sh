#!/usr/bin/env bash
set -u
NEXT_ID=${NEXT_ID:-$(cat /tmp/cognition_bench_next_run_id)}
RESULT_DIR="results/cognition_bench/$NEXT_ID"
LOG_DIR="logs/$NEXT_ID"
PID_FILE="$LOG_DIR/pids.txt"
mkdir -p "$RESULT_DIR" "$LOG_DIR"
: > "$PID_FILE"
launch() {
  local name="$1"; shift
  local limit="$1"; shift
  echo "launch $name limit=$limit $*"
  nohup timeout "$limit" taskset -c 48-67 ./target/release/cognition_bench "$@" > "$LOG_DIR/$name.log" 2>&1 &
  echo "$! $name" >> "$PID_FILE"
  sleep 0.15
}
# Harder correctness branch: repeat non-adaptation reasoning under live load.
for seed in 70000 70001 70002 70003; do
  launch "qa-depth_max_seed${seed}" 2h --case qa-depth --scale max --seed "$seed" --threads 20 --out "$RESULT_DIR/qa-depth_max_seed${seed}.jsonl"
done
for seed in 70100 70101 70102 70103; do
  launch "ablation-matrix_max_seed${seed}" 90m --case ablation-matrix --scale max --seed "$seed" --threads 20 --out "$RESULT_DIR/ablation-matrix_max_seed${seed}.jsonl"
done
for seed in 70200 70201 70202 70203; do
  launch "memory-pressure_medium_seed${seed}" 2h --case memory-pressure --scale medium --seed "$seed" --threads 20 --out "$RESULT_DIR/memory-pressure_medium_seed${seed}.jsonl"
done
for seed in 70300 70301; do
  launch "chaos-run_medium_seed${seed}" 90m --case chaos-run --scale medium --duration-minutes 60 --seed "$seed" --threads 20 --out "$RESULT_DIR/chaos-run_medium_seed${seed}.jsonl"
done
echo "cluster=$NEXT_ID launched=$(wc -l < "$PID_FILE") pid_file=$PID_FILE"
