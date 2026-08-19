#!/usr/bin/env python3
"""Independent externality audit for the frozen external mathematics release.

This script intentionally does not import the Rust crate or any benchmark
schema.  It reads only the release manifest, question JSONL, source files, and
repository text.  Sealed oracle bytes are hashed for manifest verification but
never parsed as answers.
"""

from __future__ import annotations

import hashlib
import json
import re
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RELEASE = ROOT / "data" / "external_math_exam_v1"
MANIFEST_PATH = RELEASE / "manifest.json"
QUESTIONS_PATH = RELEASE / "questions.jsonl"
REPORT_JSON = ROOT / "docs" / "stage346_externality_audit.json"
REPORT_MD = ROOT / "docs" / "stage346_externality_audit.md"


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def normalize(value: str) -> str:
    return re.sub(r"\s+", " ", value.strip().lower())


def template_signature(value: str) -> str:
    value = normalize(value)
    value = re.sub(r"\\[a-zA-Z]+", "<latex>", value)
    value = re.sub(r"\$+", "<math>", value)
    value = re.sub(r"-?\d+(?:\.\d+)?", "<num>", value)
    value = re.sub(r"\b[a-zA-Z]\b", "<sym>", value)
    value = re.sub(r"[^a-z0-9<> ]+", " ", value)
    return re.sub(r"\s+", " ", value).strip()


def load_questions() -> list[dict[str, str]]:
    records = []
    for line in QUESTIONS_PATH.read_text(encoding="utf-8").splitlines():
        if line.strip():
            record = json.loads(line)
            required = {"id", "original_prompt", "split", "source_id", "source_item_id"}
            missing = required.difference(record)
            if missing:
                raise ValueError(f"question {record.get('id')} missing {sorted(missing)}")
            records.append(record)
    return records


def repository_texts() -> list[tuple[str, str]]:
    chunks: list[tuple[str, str]] = []
    # Audit against implementation and source material only.  Evaluation
    # reports necessarily contain prompt IDs and hashes, so including all
    # docs/data would manufacture overlap after the first run.
    roots = [ROOT / "src", ROOT / "docs" / "sources"]
    excluded = {
        QUESTIONS_PATH.resolve(),
        (RELEASE / "oracle_development.jsonl").resolve(),
        (RELEASE / "oracle_sealed.jsonl").resolve(),
        REPORT_JSON.resolve(),
        REPORT_MD.resolve(),
    }
    for root in roots:
        for path in sorted(root.rglob("*")):
            if not path.is_file() or path.resolve() in excluded:
                continue
            if path.suffix.lower() not in {".rs", ".txt", ".md", ".json", ".jsonl"}:
                continue
            # Avoid treating large curriculum/memory stores as source text;
            # those are separately hashed and are not externality evidence.
            if path.stat().st_size > 2_000_000:
                continue
            try:
                chunks.append(
                    (str(path.relative_to(ROOT)), path.read_text(encoding="utf-8", errors="ignore"))
                )
            except OSError:
                continue
    return chunks


def main() -> None:
    manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    questions = load_questions()
    if len(questions) != manifest["selection"]["development_count"] + manifest["selection"]["sealed_count"]:
        raise ValueError("question count does not match release manifest")

    file_hashes = {
        name: sha256_file(RELEASE / name) for name in manifest["files"]
    }
    manifest_hashes_match = all(
        file_hashes[name] == expected for name, expected in manifest["files"].items()
    )

    normalized = {record["id"]: normalize(record["original_prompt"]) for record in questions}
    prompt_hashes = {record_id: sha256_bytes(value.encode()) for record_id, value in normalized.items()}
    by_split: dict[str, list[str]] = defaultdict(list)
    for record in questions:
        by_split[record["split"]].append(prompt_hashes[record["id"]])
    split_intersections = {}
    split_names = sorted(by_split)
    for left_index, left in enumerate(split_names):
        for right in split_names[left_index + 1 :]:
            split_intersections[f"{left}::{right}"] = len(
                set(by_split[left]).intersection(by_split[right])
            )

    duplicate_groups = Counter(prompt_hashes.values())
    duplicate_prompt_groups = sum(count > 1 for count in duplicate_groups.values())
    duplicate_prompt_records = sum(count for count in duplicate_groups.values() if count > 1)

    signatures = Counter(template_signature(record["original_prompt"]) for record in questions)
    repeated_template_groups = sum(count > 1 for count in signatures.values())
    repeated_template_records = sum(count for count in signatures.values() if count > 1)

    answer_marker_pattern = re.compile(r"####\s*[-+]?\d")
    answer_marker_records = [
        record["id"]
        for record in questions
        if answer_marker_pattern.search(record["original_prompt"])
    ]

    repository_text_by_path = [
        (path, normalize(text)) for path, text in repository_texts()
    ]
    repository_overlap_matches = {}
    for record in questions:
        prompt = normalize(record["original_prompt"])
        matches = [path for path, text in repository_text_by_path if prompt in text]
        if matches:
            repository_overlap_matches[record["id"]] = matches
    exact_repository_overlaps = sorted(repository_overlap_matches)

    source_ids = Counter(record["source_id"] for record in questions)
    source_item_duplicates = sum(
        count > 1
        for count in Counter(record["source_item_id"] for record in questions).values()
    )
    expected_splits = {"development", "sealed"}
    actual_splits = {record["split"] for record in questions}
    structural_checks = {
        "manifest_schema": manifest.get("schema") == "external-math-exam-release-v1",
        "holdout_locked": manifest.get("holdout_locked") is True,
        "original_wording_preserved": manifest["selection"].get("original_wording_preserved") is True,
        "answers_absent_from_questions": manifest["selection"].get("answers_in_questions") is False,
        "sealed_policy_hash_only": "plaintext sealed answers" in manifest.get("sealed_answer_policy", ""),
        "expected_splits_only": actual_splits == expected_splits,
        "manifest_file_hashes_match": manifest_hashes_match,
    }
    clean_checks = {
        "cross_split_exact_overlap": all(value == 0 for value in split_intersections.values()),
        "duplicate_prompt_groups": duplicate_prompt_groups == 0,
        "answer_markers": len(answer_marker_records) == 0,
        "repository_exact_overlap": len(exact_repository_overlaps) == 0,
    }
    externality_verdict = (
        "clean_under_static_audit"
        if all(structural_checks.values()) and all(clean_checks.values())
        else "review_required"
    )

    report = {
        "schema": "stage346-independent-externality-audit-v1",
        "release_id": manifest.get("release_id"),
        "manifest_sha256": sha256_file(MANIFEST_PATH),
        "questions_sha256": sha256_file(QUESTIONS_PATH),
        "oracle_development_sha256": file_hashes.get("oracle_development.jsonl"),
        "oracle_sealed_sha256": file_hashes.get("oracle_sealed.jsonl"),
        "questions": len(questions),
        "split_counts": dict(Counter(record["split"] for record in questions)),
        "source_ids": dict(source_ids),
        "source_item_duplicate_groups": source_item_duplicates,
        "split_exact_intersections": split_intersections,
        "duplicate_prompt_groups": duplicate_prompt_groups,
        "duplicate_prompt_records": duplicate_prompt_records,
        "repeated_template_groups": repeated_template_groups,
        "repeated_template_records": repeated_template_records,
        "answer_marker_records": answer_marker_records,
        "repository_exact_overlap_records": exact_repository_overlaps,
        "repository_exact_overlap_matches": repository_overlap_matches,
        "structural_checks": structural_checks,
        "clean_checks": clean_checks,
        "externality_verdict": externality_verdict,
        "sealed_answers_parsed": False,
        "implementation_mutations": 0,
        "report_sha256": "",
    }
    report["report_sha256"] = sha256_bytes(
        json.dumps({**report, "report_sha256": ""}, sort_keys=True).encode()
    )
    REPORT_JSON.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    REPORT_MD.write_text(
        "# Stage 346 — independent externality audit\n\n"
        f"* release / questions: `{report['release_id']}` / {report['questions']}\n"
        f"* split counts: `{report['split_counts']}`\n"
        f"* cross-split exact overlaps: {sum(split_intersections.values())}\n"
        f"* duplicate prompts: {duplicate_prompt_groups} groups / {duplicate_prompt_records} records\n"
        f"* repeated normalized templates: {repeated_template_groups} groups / {repeated_template_records} records\n"
        f"* answer markers: {len(answer_marker_records)}\n"
        f"* repository exact prompt overlaps: {len(exact_repository_overlaps)}\n"
        f"* overlap locations: `{repository_overlap_matches}`\n"
        f"* sealed answers parsed: false\n"
        f"* verdict: **{externality_verdict}**\n\n"
        "This evaluator is implemented independently of the Rust benchmark and curriculum schemas. "
        "It verifies release hashes, split isolation, wording/provenance metadata, exact and "
        "normalized overlap indicators, and answer-marker exposure. Oracle contents are hashed "
        "for release integrity but never parsed. Repeated templates are reported as a diagnostic "
        "signal, not automatically treated as contamination.\n",
        encoding="utf-8",
    )
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
