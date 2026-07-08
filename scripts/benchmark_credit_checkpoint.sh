#!/usr/bin/env bash
set -euo pipefail
mode="${1:-periodic}"
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
out_dir="docs/benchmark_runs/credit_window"
log_dir="logs/credit_window"
mkdir -p "$out_dir" "$log_dir"
summary="$out_dir/checkpoint_${stamp}_${mode}.md"
json="$log_dir/checkpoint_${stamp}_${mode}.json"

python3 - "$mode" "$summary" "$json" <<'PY'
import json, os, re, subprocess, sys, time
from pathlib import Path
mode, summary_path, json_path = sys.argv[1:4]
now = time.strftime('%Y-%m-%d %H:%M:%S UTC', time.gmtime())
ps = subprocess.check_output(['ps','-eo','pid,ppid,pgid,etimes,pcpu,pmem,cmd'], text=True)
ncpu = os.cpu_count() or 1
used_cpu = 0.0
active = []
for line in ps.splitlines()[1:]:
    parts = line.split(None, 6)
    if len(parts) < 7:
        continue
    pid, ppid, pgid, etimes, pcpu, pmem, cmd = parts
    try:
        used_cpu += float(pcpu) / 100.0
    except ValueError:
        pass
    if 'timeout ' in cmd and ('cognition_bench' in cmd or 'chess_stockfish_selfplay' in cmd):
        m = re.search(r'timeout (\d+)([hm])', cmd)
        limit = None
        if m:
            limit = int(m.group(1)) * (60 if m.group(2) == 'h' else 1)
        elapsed = int(etimes) / 60.0
        rem = None if limit is None else max(0.0, limit - elapsed)
        case = 'chess' if 'chess_stockfish' in cmd else (re.search(r'--case ([^ ]+)', cmd).group(1) if re.search(r'--case ([^ ]+)', cmd) else '?')
        scale = '-' if case == 'chess' else (re.search(r'--scale ([^ ]+)', cmd).group(1) if re.search(r'--scale ([^ ]+)', cmd) else '?')
        run = re.search(r'results/(?:cognition_bench|chess_stockfish)/([^/]+)/', cmd)
        active.append({'pid': int(pid), 'remaining_minutes': rem, 'elapsed_minutes': elapsed, 'pcpu': float(pcpu), 'pmem': float(pmem), 'case': case, 'scale': scale, 'run': run.group(1) if run else '?'})
results = []
base = Path('results/cognition_bench')
if base.exists():
    for d in sorted(p for p in base.iterdir() if p.is_dir()):
        files = list(d.glob('*.jsonl'))
        lines = 0
        bytes_ = 0
        for f in files:
            try:
                bytes_ += f.stat().st_size
                with f.open('rb') as fh:
                    lines += sum(1 for _ in fh)
            except OSError:
                pass
        if files:
            results.append({'run': d.name, 'files': len(files), 'lines': lines, 'bytes': bytes_})
chess_path = Path('results/chess_stockfish/20260706T143421Z_chess_stockfish_18c_4h/selfplay.jsonl')
chess_lines = 0
if chess_path.exists():
    with chess_path.open('rb') as fh:
        chess_lines = sum(1 for _ in fh)
free_est = max(0.0, ncpu - used_cpu)
payload = {'mode': mode, 'timestamp_utc': now, 'nproc': ncpu, 'used_cpu_cores_estimate': round(used_cpu, 2), 'free_cpu_cores_estimate': round(free_est, 2), 'active': active, 'result_runs': results, 'chess_lines': chess_lines}
Path(json_path).write_text(json.dumps(payload, indent=2) + '\n')
lines = []
lines.append(f'# Credit Window Checkpoint ({mode})')
lines.append('')
lines.append(f'- Timestamp: `{now}`')
lines.append(f'- Estimated CPU use: `{used_cpu:.1f}/{ncpu}` cores')
lines.append(f'- Estimated idle capacity: `{free_est:.1f}` cores')
lines.append(f'- Active timed benchmark wrappers: `{len(active)}`')
lines.append(f'- Cognition result clusters: `{len(results)}`')
lines.append(f'- Chess self-play rows: `{chess_lines}`')
lines.append('')
lines.append('## Active Long Runs')
for item in sorted(active, key=lambda x: (-1 if x['remaining_minutes'] is None else x['remaining_minutes']), reverse=True)[:40]:
    rem = '?' if item['remaining_minutes'] is None else f"{item['remaining_minutes']:.1f}m"
    lines.append(f"- pid `{item['pid']}`: `{item['case']}/{item['scale']}` in `{item['run']}`, remaining `{rem}`, cpu `{item['pcpu']:.1f}%`")
lines.append('')
lines.append('## Result Inventory')
for r in results:
    lines.append(f"- `{r['run']}`: `{r['files']}` files, `{r['lines']}` rows, `{r['bytes']}` bytes")
Path(summary_path).write_text('\n'.join(lines) + '\n')
PY

git add scripts docs/benchmark_runs logs results src experiments Cargo.toml Cargo.lock README.md CURRENT_STATE.md MATH.md GUIDE.md UPDATES.md tests 2>/dev/null || true
if ! git diff --cached --quiet; then
  git commit -m "docs: checkpoint benchmark credit window ${stamp} ${mode}" >> "$log_dir/git_${stamp}_${mode}.log" 2>&1 || true
  git push >> "$log_dir/git_${stamp}_${mode}.log" 2>&1 || true
fi
