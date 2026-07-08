#!/usr/bin/env bash
set -u
ADAPT_ID=${ADAPT_ID:-$(cat /tmp/cognition_bench_adaptation_diag_run_id)}
RESULT_DIR="results/cognition_bench/$ADAPT_ID"
LOG_DIR="logs/$ADAPT_ID"
PID_FILE="$LOG_DIR/pids.txt"
mkdir -p "$RESULT_DIR" "$LOG_DIR"
: > "$PID_FILE"
launch() {
  local name="$1"; shift
  local limit="$1"; shift
  echo "launch $name limit=$limit $*"
  nohup timeout "$limit" taskset -c 33-47 ./target/release/cognition_bench "$@" > "$LOG_DIR/$name.log" 2>&1 &
  echo "$! $name" >> "$PID_FILE"
  sleep 0.2
}
# Small controls: verify adaptation can still complete under current load.
for seed in 61000 61001 61002; do
  launch "adaptation_small_control_seed${seed}" 20m --case adaptation --scale small --seed "$seed" --threads 8 --out "$RESULT_DIR/adaptation_small_control_seed${seed}.jsonl"
done
# Medium profile: bounded diagnostic jobs for the unresolved adaptation scaling path.
for seed in 61100 61101 61102 61103 61104 61105; do
  launch "adaptation_medium_profile_seed${seed}" 90m --case adaptation --scale medium --seed "$seed" --threads 12 --out "$RESULT_DIR/adaptation_medium_profile_seed${seed}.jsonl"
done
# Large sentinels: long capped probes to see if any large adaptation emits JSONL.
for seed in 61200 61201; do
  launch "adaptation_large_sentinel_seed${seed}" 3h --case adaptation --scale large --seed "$seed" --threads 12 --out "$RESULT_DIR/adaptation_large_sentinel_seed${seed}.jsonl"
done
echo "cluster=$ADAPT_ID launched=$(wc -l < "$PID_FILE") pid_file=$PID_FILE"
