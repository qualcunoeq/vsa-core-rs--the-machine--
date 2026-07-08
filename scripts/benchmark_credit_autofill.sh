#!/usr/bin/env bash
set -euo pipefail
remaining_to_final_min="${1:-60}"
stamp="$(date -u +%Y%m%dT%H%M%SZ)_autofill"
out_dir="results/cognition_bench/${stamp}"
log_dir="logs/${stamp}"
mkdir -p "$out_dir" "$log_dir"
free_cores="$(python3 - <<'PY'
import os, subprocess
ps = subprocess.check_output(['ps','-eo','pcpu,cmd'], text=True)
used = 0.0
for line in ps.splitlines()[1:]:
    parts = line.strip().split(None, 1)
    if not parts:
        continue
    try:
        used += float(parts[0]) / 100.0
    except ValueError:
        pass
print(max(0, int((os.cpu_count() or 1) - used)))
PY
)"
if [ "$remaining_to_final_min" -lt 45 ]; then
  echo "skip: less than 45 minutes before final checkpoint" | tee "$log_dir/decision.log"
  exit 0
fi
if [ "$free_cores" -lt 6 ]; then
  echo "skip: only ${free_cores} estimated free cores" | tee "$log_dir/decision.log"
  exit 0
fi
# Keep every opportunistic run bounded so it can finish before the final checkpoint.
duration=90
if [ "$remaining_to_final_min" -gt 190 ]; then duration=180; fi
if [ "$remaining_to_final_min" -gt 130 ] && [ "$duration" -lt 120 ]; then duration=120; fi
slots=$((free_cores - 4))
if [ "$slots" -gt 34 ]; then slots=34; fi
if [ "$slots" -lt 4 ]; then slots=4; fi
launched=0
# High-signal only: these runs expose learning/adaptation, state pressure, and robustness.
cases=("adaptation:large" "memory-pressure:large" "chaos-run:large" "adaptation:large" "memory-pressure:large" "chaos-run:large")
for ((i=0; i<slots; i++)); do
  pair="${cases[$((i % ${#cases[@]}))]}"
  case_name="${pair%%:*}"
  scale="${pair##*:}"
  seed=$((99000 + i))
  out="$out_dir/${case_name}_${scale}_autofill_seed${seed}.jsonl"
  log="$log_dir/${case_name}_${scale}_autofill_seed${seed}.log"
  timeout "${duration}m" ./target/release/cognition_bench --case "$case_name" --scale "$scale" --seed "$seed" --threads 80 --out "$out" > "$log" 2>&1 &
  echo "$! $case_name $scale $seed $duration $out" >> "$log_dir/pids.txt"
  launched=$((launched + 1))
done
cat > "docs/benchmark_runs/${stamp}.md" <<EOF
# ${stamp}

Autofill benchmark wave launched by the credit-window orchestrator.

- Timestamp: $(date -u '+%Y-%m-%d %H:%M:%S UTC')
- Estimated free cores before launch: ${free_cores}
- Remaining minutes before final checkpoint: ${remaining_to_final_min}
- Per-job timeout: ${duration}m
- Jobs launched: ${launched}
- Cases: adaptation/large, memory-pressure/large, chaos-run/large only
EOF
echo "launched ${launched} jobs for ${duration}m with ${free_cores} estimated free cores" | tee "$log_dir/decision.log"
