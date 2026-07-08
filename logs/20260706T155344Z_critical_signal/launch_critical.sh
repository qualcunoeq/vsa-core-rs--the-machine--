#!/usr/bin/env bash
set -u
CRIT_ID=${CRIT_ID:-$(cat /tmp/cognition_bench_critical_run_id)}
RESULT_DIR="results/cognition_bench/$CRIT_ID"
LOG_DIR="logs/$CRIT_ID"
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
# 1. Adaptation is the main unresolved bottleneck: compare medium/large under a fresh critical branch.
for seed in 90000 90001 90002 90003; do
  launch "adaptation_medium_critical_seed${seed}" 90m --case adaptation --scale medium --seed "$seed" --threads 80 --out "$RESULT_DIR/adaptation_medium_critical_seed${seed}.jsonl"
done
for seed in 90100 90101; do
  launch "adaptation_large_critical_seed${seed}" 3h --case adaptation --scale large --seed "$seed" --threads 80 --out "$RESULT_DIR/adaptation_large_critical_seed${seed}.jsonl"
done
# 2. Large memory pressure is the clearest scaling bottleneck.
for seed in 90200 90201 90202 90203 90204 90205; do
  launch "memory-pressure_large_critical_seed${seed}" 3h --case memory-pressure --scale large --seed "$seed" --threads 80 --out "$RESULT_DIR/memory-pressure_large_critical_seed${seed}.jsonl"
done
# 3. Deep reasoning under live load.
for seed in 90300 90301 90302 90303; do
  launch "qa-depth_max_critical_seed${seed}" 2h --case qa-depth --scale max --seed "$seed" --threads 80 --out "$RESULT_DIR/qa-depth_max_critical_seed${seed}.jsonl"
done
# 4. Chaos durability: long enough to catch panics without monopolizing the box forever.
for seed in 90400 90401 90402 90403; do
  launch "chaos-run_medium_critical_seed${seed}" 90m --case chaos-run --scale medium --duration-minutes 60 --seed "$seed" --threads 80 --out "$RESULT_DIR/chaos-run_medium_critical_seed${seed}.jsonl"
done
echo "cluster=$CRIT_ID launched=$(wc -l < "$PID_FILE") pid_file=$PID_FILE"
