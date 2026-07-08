#!/usr/bin/env bash
set -u
CLUSTER_ID=${CLUSTER_ID:-$(cat /tmp/cognition_bench_cluster2_run_id)}
RESULT_DIR="results/cognition_bench/$CLUSTER_ID"
LOG_DIR="logs/$CLUSTER_ID"
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
# Broad medium adaptation stability: many seeds, expected to complete and expose regressions.
for seed in $(seq 40000 40039); do
  launch "adaptation_medium_seed${seed}" 75m --case adaptation --scale medium --seed "$seed" --threads 80 --out "$RESULT_DIR/adaptation_medium_seed${seed}.jsonl"
done
# Memory-pressure concurrency cluster: known latency bottleneck at 10k facts.
for seed in $(seq 41000 41031); do
  launch "memory-pressure_medium_seed${seed}" 2h --case memory-pressure --scale medium --seed "$seed" --threads 80 --out "$RESULT_DIR/memory-pressure_medium_seed${seed}.jsonl"
done
# Larger memory probes: fewer jobs because 100k facts was the worst completed latency path.
for seed in $(seq 42000 42005); do
  launch "memory-pressure_large_seed${seed}" 3h --case memory-pressure --scale large --seed "$seed" --threads 80 --out "$RESULT_DIR/memory-pressure_large_seed${seed}.jsonl"
done
# Long-capped adaptation-large probes: the previous large/max attempts produced no JSONL.
for seed in $(seq 43000 43001); do
  launch "adaptation_large_probe_seed${seed}" 4h --case adaptation --scale large --seed "$seed" --threads 80 --out "$RESULT_DIR/adaptation_large_probe_seed${seed}.jsonl"
done
# Extra deep QA probes to correlate chain depth with latency under concurrent pressure.
for seed in $(seq 44000 44007); do
  launch "qa-depth_max_seed${seed}" 2h --case qa-depth --scale max --seed "$seed" --threads 80 --out "$RESULT_DIR/qa-depth_max_seed${seed}.jsonl"
done

echo "cluster=$CLUSTER_ID launched=$(wc -l < "$PID_FILE") pid_file=$PID_FILE"
