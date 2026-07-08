# Raw Results Recovery 2026-07-07

The Brev instance was restarted after credits were replenished. The raw benchmark files survived on disk and were force-added because `.gitignore` ignores `*.jsonl` and `*.log`.

- Recovery timestamp: 2026-07-07 06:18:26 UTC
- Raw result JSONL files recovered: 617
- Raw result JSONL rows recovered: 11099
- Raw result JSONL bytes recovered: 4415260
- Log files discovered: 865
- Chess self-play rows: 8251
- Latest pre-recovery checkpoint: `docs/benchmark_runs/credit_window/checkpoint_20260707T013625Z_periodic.md`
- Note: no final checkpoint was written before the credit interruption; the last pushed checkpoint was periodic.

## Preservation Notes

The raw JSONL files are the most important recovery artifact. They contain the per-run benchmark output needed for later analysis. The checkpoint docs remain useful as summaries, but they are not a substitute for the raw result files.
