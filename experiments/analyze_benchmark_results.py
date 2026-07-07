#!/usr/bin/env python3
"""Summarize cognition and chess benchmark JSONL outputs.

This is deliberately lightweight: it consumes the raw artifacts already written
by the benchmark runners and emits a Markdown report suitable for committing.
"""
from __future__ import annotations

import argparse
import json
import math
import statistics
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any


def percentile(values: list[float], pct: float) -> float | None:
    clean = sorted(v for v in values if math.isfinite(v))
    if not clean:
        return None
    k = (len(clean) - 1) * pct / 100.0
    lo = math.floor(k)
    hi = math.ceil(k)
    if lo == hi:
        return clean[int(k)]
    return clean[lo] * (hi - k) + clean[hi] * (k - lo)


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8", errors="replace") as handle:
        for lineno, line in enumerate(handle, 1):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                rows.append(
                    {
                        "kind": "parse_error",
                        "path": str(path),
                        "line": lineno,
                        "error": str(exc),
                    }
                )
                continue
            row["_path"] = str(path)
            rows.append(row)
    return rows


def collect_jsonl(root: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    if root.exists():
        for path in sorted(root.rglob("*.jsonl")):
            rows.extend(load_jsonl(path))
    return rows


def summarize_cognition(rows: list[dict[str, Any]]) -> list[str]:
    lines = ["## Cognition Benchmarks", ""]
    result_rows = [r for r in rows if "experiment" in r and isinstance(r.get("metrics"), dict)]
    parse_errors = [r for r in rows if r.get("kind") == "parse_error"]
    passed = sum(1 for r in result_rows if r.get("passed") is True)
    failed = sum(1 for r in result_rows if r.get("passed") is False)

    lines.extend(
        [
            f"- Rows: `{len(result_rows)}`",
            f"- Passed: `{passed}`",
            f"- Failed: `{failed}`",
            f"- Parse errors: `{len(parse_errors)}`",
            "",
        ]
    )

    by_experiment: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in result_rows:
        by_experiment[str(row.get("experiment"))].append(row)

    lines.append("| Experiment | Rows | Passed | Failed | Avg Accuracy | P95 Latency ms | Notes |")
    lines.append("| --- | ---: | ---: | ---: | ---: | ---: | --- |")
    for experiment in sorted(by_experiment):
        group = by_experiment[experiment]
        accuracies = [
            float(row["metrics"]["accuracy"])
            for row in group
            if isinstance(row.get("metrics", {}).get("accuracy"), (int, float))
        ]
        latencies = [
            float(row["metrics"]["p95_latency_ms"])
            for row in group
            if isinstance(row.get("metrics", {}).get("p95_latency_ms"), (int, float))
        ]
        notes = Counter(str(row.get("notes", "")) for row in group).most_common(1)
        avg_accuracy = statistics.mean(accuracies) if accuracies else None
        p95_latency = percentile(latencies, 95)
        lines.append(
            "| {} | {} | {} | {} | {} | {} | {} |".format(
                experiment,
                len(group),
                sum(1 for row in group if row.get("passed") is True),
                sum(1 for row in group if row.get("passed") is False),
                f"{avg_accuracy:.4f}" if avg_accuracy is not None else "",
                f"{p95_latency:.2f}" if p95_latency is not None else "",
                notes[0][0].replace("|", "\\|") if notes else "",
            )
        )

    lines.append("")
    lines.append("### Metric Ranges")
    metric_values: dict[str, list[float]] = defaultdict(list)
    for row in result_rows:
        for key, value in row.get("metrics", {}).items():
            if isinstance(value, (int, float)) and math.isfinite(float(value)):
                metric_values[key].append(float(value))
    lines.append("| Metric | N | Min | P50 | P95 | Max |")
    lines.append("| --- | ---: | ---: | ---: | ---: | ---: |")
    for key in sorted(metric_values):
        values = metric_values[key]
        lines.append(
            "| {} | {} | {:.4f} | {:.4f} | {:.4f} | {:.4f} |".format(
                key,
                len(values),
                min(values),
                percentile(values, 50) or 0.0,
                percentile(values, 95) or 0.0,
                max(values),
            )
        )
    lines.append("")
    return lines


def summarize_chess(rows: list[dict[str, Any]]) -> list[str]:
    lines = ["## Chess Self-Play", ""]
    if not rows:
        lines.extend(["No chess rows found.", ""])
        return lines

    kinds = Counter(str(row.get("kind")) for row in rows)
    progress = [row for row in rows if row.get("kind") == "progress"]
    done = [row for row in rows if row.get("kind") == "run_done"]
    errors = [row for row in rows if row.get("kind") == "worker_error"]

    lines.extend(
        [
            f"- Rows: `{len(rows)}`",
            f"- Progress rows: `{len(progress)}`",
            f"- Worker errors: `{len(errors)}`",
            f"- Run done marker: `{bool(done)}`",
        ]
    )
    if done:
        lines.append(f"- Games: `{int(done[-1].get('games', 0))}`")
        lines.append(f"- Plies: `{int(done[-1].get('plies', 0))}`")
    lines.append("")
    lines.append("| Kind | Count |")
    lines.append("| --- | ---: |")
    for kind, count in kinds.most_common():
        lines.append(f"| {kind} | {count} |")

    if progress:
        first = progress[0]
        last = progress[-1]
        interval_agreements = [
            float(row["interval_agreement_rate"])
            for row in progress
            if isinstance(row.get("interval_agreement_rate"), (int, float))
        ]
        cumulative = [
            float(row["agreement_rate"])
            for row in progress
            if isinstance(row.get("agreement_rate"), (int, float))
        ]
        lines.extend(
            [
                "",
                "### Learning Signal",
                f"- First interval agreement: `{first.get('interval_agreement_rate')}`",
                f"- Last interval agreement: `{last.get('interval_agreement_rate')}`",
                f"- Best interval agreement: `{max(interval_agreements):.4f}`",
                f"- Mean interval agreement: `{statistics.mean(interval_agreements):.4f}`",
                f"- First cumulative agreement: `{first.get('agreement_rate')}`",
                f"- Last cumulative agreement: `{last.get('agreement_rate')}`",
                f"- Best cumulative agreement: `{max(cumulative):.4f}`",
                "",
            ]
        )
    return lines


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--results", type=Path, default=Path("results"))
    parser.add_argument("--out", type=Path, default=Path("docs/benchmark_runs/analysis_latest.md"))
    args = parser.parse_args()

    cognition = collect_jsonl(args.results / "cognition_bench")
    chess = collect_jsonl(args.results / "chess_stockfish")

    lines = [
        "# Benchmark Analysis",
        "",
        "Generated from recovered raw JSONL artifacts.",
        "",
    ]
    lines.extend(summarize_cognition(cognition))
    lines.extend(summarize_chess(chess))

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(args.out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
