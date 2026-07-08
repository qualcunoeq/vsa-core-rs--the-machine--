#!/usr/bin/env bash
set -u
MANUAL_ID=${MANUAL_ID:-$(cat /tmp/cognition_bench_manual_priority_run_id)}
RESULT_DIR="results/cognition_bench/$MANUAL_ID"
LOG_DIR="logs/$MANUAL_ID"
PID_FILE="$LOG_DIR/pids.txt"
mkdir -p "$RESULT_DIR" "$LOG_DIR"
: > "$PID_FILE"
launch() {
  local name="$1"; shift
  local limit="$1"; shift
  echo "launch $name limit=$limit $*"
  nohup timeout "$limit" ./target/release/cognition_bench "$@" > "$LOG_DIR/$name.log" 2>&1 &
  echo "$! $name" >> "$PID_FILE"
  sleep 0.08
}
# Manual priority batch: use idle cores now, but avoid excessive duplication of the active memory ladder.
for seed in 96000 96001 96002 96003; do
  launch "adaptation_medium_manual_seed${seed}" 90m --case adaptation --scale medium --seed "$seed" --threads 80 --out "$RESULT_DIR/adaptation_medium_manual_seed${seed}.jsonl"
done
for seed in 96100 96101; do
  launch "adaptation_large_manual_seed${seed}" 4h --case adaptation --scale large --seed "$seed" --threads 80 --out "$RESULT_DIR/adaptation_large_manual_seed${seed}.jsonl"
done
for seed in 96200 96201 96202 96203; do
  launch "chaos-run_medium_manual_seed${seed}" 90m --case chaos-run --scale medium --duration-minutes 60 --seed "$seed" --threads 80 --out "$RESULT_DIR/chaos-run_medium_manual_seed${seed}.jsonl"
done
for seed in 96300 96301 96302 96303; do
  launch "ablation-matrix_max_manual_seed${seed}" 90m --case ablation-matrix --scale max --seed "$seed" --threads 80 --out "$RESULT_DIR/ablation-matrix_max_manual_seed${seed}.jsonl"
done
for seed in 96400 96401 96402 96403; do
  launch "qa-depth_max_manual_seed${seed}" 2h --case qa-depth --scale max --seed "$seed" --threads 80 --out "$RESULT_DIR/qa-depth_max_manual_seed${seed}.jsonl"
done
echo "cluster=$MANUAL_ID launched=$(wc -l < "$PID_FILE") pid_file=$PID_FILE"
