#!/usr/bin/env bash
set -euo pipefail
duration_minutes="${1:-492}"
interval_minutes="${2:-10}"
final_margin_minutes="${3:-10}"
log_dir="logs/credit_window"
mkdir -p "$log_dir"

existing_final=""
existing_deadline=""
if [ -f "$log_dir/orchestrator_state.env" ]; then
  existing_final="$(grep '^final_checkpoint_utc=' "$log_dir/orchestrator_state.env" | cut -d= -f2- || true)"
  existing_deadline="$(grep '^deadline_utc=' "$log_dir/orchestrator_state.env" | cut -d= -f2- || true)"
fi
if [ -n "$existing_final" ]; then
  final_ts="$(date -u -d "$existing_final" +%s)"
  if [ -n "$existing_deadline" ]; then
    deadline_ts="$(date -u -d "$existing_deadline" +%s)"
  else
    deadline_ts=$((final_ts + final_margin_minutes * 60))
  fi
else
  start_ts="$(date +%s)"
  deadline_ts=$((start_ts + duration_minutes * 60))
  final_ts=$((deadline_ts - final_margin_minutes * 60))
fi

echo $$ > "$log_dir/orchestrator.pid"
{
  echo "start_utc=$(date -u '+%Y-%m-%d %H:%M:%S UTC')"
  echo "duration_minutes=${duration_minutes}"
  echo "interval_minutes=${interval_minutes}"
  echo "final_margin_minutes=${final_margin_minutes}"
  echo "final_checkpoint_utc=$(date -u -d @${final_ts} '+%Y-%m-%d %H:%M:%S UTC')"
  echo "deadline_utc=$(date -u -d @${deadline_ts} '+%Y-%m-%d %H:%M:%S UTC')"
} > "$log_dir/orchestrator_state.env"

scripts/benchmark_credit_checkpoint.sh start || true
while true; do
  now="$(date +%s)"
  if [ "$now" -ge "$final_ts" ]; then
    break
  fi
  remaining_to_final=$(((final_ts - now) / 60))
  scripts/benchmark_credit_autofill.sh "$remaining_to_final" || true
  scripts/benchmark_credit_checkpoint.sh periodic || true
  now="$(date +%s)"
  sleep_seconds=$((interval_minutes * 60))
  if [ $((now + sleep_seconds)) -gt "$final_ts" ]; then
    sleep_seconds=$((final_ts - now))
  fi
  if [ "$sleep_seconds" -gt 0 ]; then
    sleep "$sleep_seconds"
  fi
done
scripts/benchmark_credit_checkpoint.sh final || true
echo "final checkpoint completed at $(date -u '+%Y-%m-%d %H:%M:%S UTC')" >> "$log_dir/orchestrator_state.env"
