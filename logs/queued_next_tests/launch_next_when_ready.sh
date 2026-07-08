#!/usr/bin/env bash
set -u
QUEUE_DIR="logs/queued_next_tests"
MONITOR="$QUEUE_DIR/queue_monitor.jsonl"
STATE="$QUEUE_DIR/state.env"
mkdir -p "$QUEUE_DIR" results/cognition_bench docs/benchmark_runs
touch "$STATE"

active_cognition_cores() {
  python3 - <<'PY'
import subprocess
try:
    out=subprocess.check_output(['ps','-C','cognition_bench','-o','pcpu='], text=True)
except subprocess.CalledProcessError:
    out=''
print(round(sum(float(x.strip()) for x in out.splitlines() if x.strip())/100, 3))
PY
}
tracked_cores() {
  python3 - <<'PY'
import subprocess
patterns=['cognition_bench','/usr/games/stockfish','chess_stockfish_selfplay.py']
try:
    ps=subprocess.check_output(['ps','-eo','pcpu=,args='], text=True)
except subprocess.CalledProcessError:
    ps=''
total=0.0
for line in ps.splitlines():
    parts=line.split(None,1)
    if len(parts) >= 2 and any(p in parts[1] for p in patterns):
        total += float(parts[0])
print(round(total/100, 3))
PY
}
state_has() { grep -q "^$1=" "$STATE" 2>/dev/null; }
state_value() { grep "^$1=" "$STATE" 2>/dev/null | tail -n1 | cut -d= -f2-; }
active_run_jobs() {
  local run_id="$1"
  ps -eo args= | grep -F "$run_id" | grep -F cognition_bench | grep -v grep | wc -l
}
all_jobs_done() {
  local run_id="$1"
  [ -n "$run_id" ] || return 0
  [ "$(active_run_jobs "$run_id")" -eq 0 ]
}
log_monitor() {
  local cognition="$1" tracked="$2" event="$3"
  python3 - <<PY >> "$MONITOR"
import json, time
tracked=float('$tracked')
print(json.dumps({'timestamp': time.time(), 'cognition_cores': float('$cognition'), 'tracked_cores': tracked, 'estimated_free_cores': round(80-tracked,3), 'event': '$event'}, sort_keys=True))
PY
}
write_doc() {
  local run_id="$1" title="$2" body="$3"
  cat > "docs/benchmark_runs/$run_id.md" <<EOF
# $title $run_id

$body

## Artifacts

- Results: \`results/cognition_bench/$run_id\`
- Logs: \`logs/$run_id\`
- PIDs: \`logs/$run_id/pids.txt\`
EOF
}
launch_job() {
  local run_id="$1" name="$2" limit="$3"; shift 3
  local result_dir="results/cognition_bench/$run_id"
  local log_dir="logs/$run_id"
  mkdir -p "$result_dir" "$log_dir"
  echo "launch $name limit=$limit $*" >> "$log_dir/launch.log"
  nohup timeout "$limit" ./target/release/cognition_bench "$@" > "$log_dir/$name.log" 2>&1 &
  echo "$! $name" >> "$log_dir/pids.txt"
  sleep 0.1
}
start_memory_scaling_ladder() {
  local run_id="$(date -u +%Y%m%dT%H%M%SZ)_memory_scaling_ladder"
  echo "MEMORY_SCALING_ID=$run_id" >> "$STATE"
  mkdir -p "results/cognition_bench/$run_id" "logs/$run_id"
  : > "logs/$run_id/pids.txt"
  write_doc "$run_id" "Memory scaling ladder run" "## Purpose

Quantify memory retrieval latency across medium, large, and max scales after current pressure drops.

## Workloads

- 4 memory-pressure medium jobs, timeout 2h.
- 4 memory-pressure large jobs, timeout 4h.
- 2 memory-pressure max jobs, timeout 6h.

## Runtime

- Starts when at least 22 tracked cores are free.
- Per-job threads: 80."
  for seed in 94000 94001 94002 94003; do
    launch_job "$run_id" "memory_medium_seed${seed}" 2h --case memory-pressure --scale medium --seed "$seed" --threads 80 --out "results/cognition_bench/$run_id/memory_medium_seed${seed}.jsonl"
  done
  for seed in 94100 94101 94102 94103; do
    launch_job "$run_id" "memory_large_seed${seed}" 4h --case memory-pressure --scale large --seed "$seed" --threads 80 --out "results/cognition_bench/$run_id/memory_large_seed${seed}.jsonl"
  done
  for seed in 94200 94201; do
    launch_job "$run_id" "memory_max_seed${seed}" 6h --case memory-pressure --scale max --seed "$seed" --threads 80 --out "results/cognition_bench/$run_id/memory_max_seed${seed}.jsonl"
  done
}
start_adaptation_isolation() {
  local run_id="$(date -u +%Y%m%dT%H%M%SZ)_adaptation_isolation"
  echo "ADAPTATION_ISOLATION_ID=$run_id" >> "$STATE"
  mkdir -p "results/cognition_bench/$run_id" "logs/$run_id"
  : > "logs/$run_id/pids.txt"
  write_doc "$run_id" "Adaptation isolation run" "## Purpose

Isolate adaptation from saturation noise and determine whether medium/large/max adaptation completes under cleaner conditions.

## Workloads

- 4 adaptation medium jobs, timeout 90m.
- 3 adaptation large jobs, timeout 4h.
- 1 adaptation max job, timeout 6h.

## Runtime

- Starts when active cognition benchmark use drops below 20 cores.
- Per-job threads: 80."
  for seed in 93000 93001 93002 93003; do
    launch_job "$run_id" "adaptation_medium_seed${seed}" 90m --case adaptation --scale medium --seed "$seed" --threads 80 --out "results/cognition_bench/$run_id/adaptation_medium_seed${seed}.jsonl"
  done
  for seed in 93100 93101 93102; do
    launch_job "$run_id" "adaptation_large_seed${seed}" 4h --case adaptation --scale large --seed "$seed" --threads 80 --out "results/cognition_bench/$run_id/adaptation_large_seed${seed}.jsonl"
  done
  launch_job "$run_id" "adaptation_max_seed93200" 6h --case adaptation --scale max --seed 93200 --threads 80 --out "results/cognition_bench/$run_id/adaptation_max_seed93200.jsonl"
}
start_low_contention_replay() {
  local run_id="$(date -u +%Y%m%dT%H%M%SZ)_low_contention_replay"
  echo "LOW_CONTENTION_ID=$run_id" >> "$STATE"
  mkdir -p "results/cognition_bench/$run_id" "logs/$run_id"
  : > "logs/$run_id/pids.txt"
  write_doc "$run_id" "Low contention replay run" "## Purpose

Replay broad cognition cases after heavy adaptation and memory jobs finish to get cleaner latency baselines.

## Workloads

- qa-depth max, ablation-matrix max, temporal-abstraction max, meta-reasoning max, autonomy-budget max.
- 4 seeds per case.

## Runtime

- Starts only after adaptation isolation and memory scaling ladder have no active jobs.
- Per-job threads: 80; timeout 90m each."
  for seed in 95000 95001 95002 95003; do
    launch_job "$run_id" "qa_depth_seed${seed}" 90m --case qa-depth --scale max --seed "$seed" --threads 80 --out "results/cognition_bench/$run_id/qa_depth_seed${seed}.jsonl"
    launch_job "$run_id" "ablation_seed${seed}" 90m --case ablation-matrix --scale max --seed "$seed" --threads 80 --out "results/cognition_bench/$run_id/ablation_seed${seed}.jsonl"
    launch_job "$run_id" "temporal_seed${seed}" 90m --case temporal-abstraction --scale max --seed "$seed" --threads 80 --out "results/cognition_bench/$run_id/temporal_seed${seed}.jsonl"
    launch_job "$run_id" "meta_seed${seed}" 90m --case meta-reasoning --scale max --seed "$seed" --threads 80 --out "results/cognition_bench/$run_id/meta_seed${seed}.jsonl"
    launch_job "$run_id" "autonomy_seed${seed}" 90m --case autonomy-budget --scale max --seed "$seed" --threads 80 --out "results/cognition_bench/$run_id/autonomy_seed${seed}.jsonl"
  done
}
run_chess_postrun_eval() {
  local chess_id="$(cat /tmp/chess_stockfish_run_id 2>/dev/null || true)"
  [ -n "$chess_id" ] || return 1
  local jsonl="results/chess_stockfish/$chess_id/selfplay.jsonl"
  [ -f "$jsonl" ] || return 1
  grep -q '"kind": "run_done"' "$jsonl" || return 1
  echo "CHESS_POSTRUN_DONE=1" >> "$STATE"
  python3 - <<'PY'
import json, pathlib
chess=pathlib.Path('/tmp/chess_stockfish_run_id').read_text().strip()
p=pathlib.Path('results/chess_stockfish')/chess/'selfplay.jsonl'
rows=[json.loads(line) for line in p.read_text().splitlines() if line.strip()]
progress=[r for r in rows if r.get('kind')=='progress']
first=progress[:18]
mid=progress[len(progress)//2:len(progress)//2+18] if progress else []
last=progress[-18:]
def avg(xs,k):
    vals=[x[k] for x in xs if k in x]
    return sum(vals)/len(vals) if vals else 0.0
latest={r['worker']:r for r in progress if 'worker' in r}
out={'chess_id':chess,'rows':len(rows),'progress_rows':len(progress),'workers':len(latest),'total_games_latest':sum(r.get('games',0) for r in latest.values()),'total_plies_latest':sum(r.get('plies',0) for r in latest.values()),'first_interval_agreement':avg(first,'interval_agreement_rate'),'middle_interval_agreement':avg(mid,'interval_agreement_rate'),'final_interval_agreement':avg(last,'interval_agreement_rate'),'final_cumulative_agreement':avg(list(latest.values()),'agreement_rate'),'first_interval_loss':avg(first,'interval_avg_loss'),'final_interval_loss':avg(last,'interval_avg_loss')}
result_path=pathlib.Path('results/chess_stockfish')/chess/'postrun_eval.json'
result_path.write_text(json.dumps(out, indent=2, sort_keys=True)+'\n')
doc=pathlib.Path('docs/benchmark_runs')/f'{chess}_postrun_eval.md'
doc.write_text(f"# Chess postrun eval {chess}\n\n- Rows: {out['rows']}\n- Progress rows: {out['progress_rows']}\n- Workers: {out['workers']}\n- Latest games: {out['total_games_latest']}\n- Latest plies: {out['total_plies_latest']}\n- First interval agreement: {out['first_interval_agreement']:.4f}\n- Middle interval agreement: {out['middle_interval_agreement']:.4f}\n- Final interval agreement: {out['final_interval_agreement']:.4f}\n- Final cumulative agreement: {out['final_cumulative_agreement']:.4f}\n- First interval loss: {out['first_interval_loss']:.4f}\n- Final interval loss: {out['final_interval_loss']:.4f}\n")
PY
}
while true; do
  cognition="$(active_cognition_cores)"
  tracked="$(tracked_cores)"
  event="poll"
  if ! state_has MEMORY_SCALING_ID; then
    if python3 - <<PY
raise SystemExit(0 if (80 - float('$tracked')) >= 22 else 1)
PY
    then
      event="start_memory_scaling_ladder"
      log_monitor "$cognition" "$tracked" "$event"
      start_memory_scaling_ladder
      tracked="$(tracked_cores)"; cognition="$(active_cognition_cores)"
    fi
  fi
  if ! state_has ADAPTATION_ISOLATION_ID; then
    if python3 - <<PY
raise SystemExit(0 if float('$cognition') < 20 else 1)
PY
    then
      event="start_adaptation_isolation"
      log_monitor "$cognition" "$tracked" "$event"
      start_adaptation_isolation
      tracked="$(tracked_cores)"; cognition="$(active_cognition_cores)"
    fi
  fi
  if state_has ADAPTATION_ISOLATION_ID && state_has MEMORY_SCALING_ID && ! state_has LOW_CONTENTION_ID; then
    adapt="$(state_value ADAPTATION_ISOLATION_ID)"
    memory="$(state_value MEMORY_SCALING_ID)"
    if all_jobs_done "$adapt" && all_jobs_done "$memory"; then
      event="start_low_contention_replay"
      log_monitor "$cognition" "$tracked" "$event"
      start_low_contention_replay
      tracked="$(tracked_cores)"; cognition="$(active_cognition_cores)"
    fi
  fi
  if ! state_has CHESS_POSTRUN_DONE; then
    if run_chess_postrun_eval; then event="chess_postrun_eval"; fi
  fi
  log_monitor "$cognition" "$tracked" "$event"
  sleep 300
done
