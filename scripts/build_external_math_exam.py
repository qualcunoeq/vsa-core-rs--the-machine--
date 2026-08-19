#!/usr/bin/env python3
"""Rebuild the frozen external MATH release from pinned parquet inputs.

The script never reads or writes plaintext answers for the sealed partition;
the oracle contains only SHA-256 hashes of the source ``extracted_solution``.
Selection is deterministic round-robin over sorted (category, level) groups.
"""

from __future__ import annotations

import argparse
import collections
import hashlib
import json
from pathlib import Path

import pandas as pd


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def select_rows(frame: pd.DataFrame, count: int, prefix: str) -> list[dict]:
    groups: dict[tuple[str, str], collections.deque[tuple[int, dict]]] = {}
    for row_index, row in frame.reset_index(drop=True).iterrows():
        record = row.to_dict()
        key = (str(record["type"]), str(record["level"]))
        groups.setdefault(key, collections.deque()).append((row_index, record))
    selected: list[dict] = []
    ordered = sorted(groups)
    while len(selected) < count:
        progressed = False
        for key in ordered:
            if len(selected) >= count:
                break
            if not groups[key]:
                continue
            progressed = True
            row_index, record = groups[key].popleft()
            selected.append(
                {
                    "category": str(record["type"]),
                    "id": f"math-v1-{prefix}-{len(selected):04d}",
                    "level": str(record["level"]),
                    "original_prompt": str(record["problem"]),
                    "source_id": "hendrycks-math-original",
                    "source_item_id": f"{prefix}-{row_index:05d}",
                    "split": prefix,
                    "_answer": str(record["extracted_solution"]).strip(),
                }
            )
        if not progressed:
            raise RuntimeError(f"source has fewer than {count} rows")
    return selected


def write_jsonl(path: Path, records: list[dict], include_answer: bool = False) -> str:
    lines = []
    for record in records:
        if include_answer:
            output = {
                "answer_sha256": sha256_bytes(record["_answer"].encode()),
                "expected_outcome": "supported",
                "id": record["id"],
            }
        else:
            output = {key: value for key, value in record.items() if key != "_answer"}
        lines.append(json.dumps(output, ensure_ascii=False, sort_keys=True))
    payload = ("\n".join(lines) + "\n").encode()
    path.write_bytes(payload)
    return sha256_bytes(payload)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--train", type=Path, required=True)
    parser.add_argument("--test", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    train = pd.read_parquet(args.train)
    test = pd.read_parquet(args.test)
    development = select_rows(train, 3000, "development")
    sealed = select_rows(test, 1000, "sealed")
    questions = development + sealed
    question_hash = write_jsonl(args.output / "questions.jsonl", questions)
    development_hash = write_jsonl(
        args.output / "oracle_development.jsonl", development, include_answer=True
    )
    sealed_hash = write_jsonl(
        args.output / "oracle_sealed.jsonl", sealed, include_answer=True
    )
    manifest = {
        "files": {
            "oracle_development.jsonl": development_hash,
            "oracle_sealed.jsonl": sealed_hash,
            "questions.jsonl": question_hash,
        },
        "holdout_locked": True,
        "release_id": "external-math-exam-v1",
        "schema": "external-math-exam-release-v1",
        "sealed_answer_policy": "Only answer hashes are stored; plaintext sealed answers are not part of the release.",
        "selection": {
            "answers_in_questions": False,
            "development_count": 3000,
            "method": "round-robin over sorted (category, level) groups",
            "original_wording_preserved": True,
            "sealed_count": 1000,
        },
        "source": {
            "citation": "Dan Hendrycks et al., Measuring Mathematical Problem Solving With the MATH Dataset, NeurIPS 2021",
            "dataset": "MATH (Hendrycks et al., 2021)",
            "dataset_locator": "https://huggingface.co/datasets/jeggers/competition_math/tree/bc97b222f353525403ba0c2b5a44df44759950d1/original",
            "license": "MIT",
            "repository": "https://github.com/hendrycks/math",
            "revision": "bc97b222f353525403ba0c2b5a44df44759950d1",
            "source_test_sha256": sha256_bytes(args.test.read_bytes()),
            "source_train_sha256": sha256_bytes(args.train.read_bytes()),
        },
    }
    (args.output / "manifest.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    print(json.dumps(manifest, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
