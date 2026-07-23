#!/usr/bin/env python3
"""Generate an independent prose-recurrence corpus and oracle labels."""

import json
from pathlib import Path


def value_at(initial: int, coefficient: int, offset: int, index: int) -> int:
    value = initial
    for _ in range(index):
        value = coefficient * value + offset
    return value


def main() -> None:
    cases = []
    pair = 0

    def add(i, category, prompt, answer=None, authorize=False, pair_id=None):
        cases.append(
            {
                "id": f"recurrence-ood-{i:04d}",
                "category": category,
                "prompt": prompt,
                "expected_route": "math",
                "expected_answer": None if answer is None else str(answer),
                "should_authorize": authorize,
                "pair_id": pair_id,
            }
        )

    i = 0
    for n in range(100):
        initial = n % 9 - 4
        coefficient = n % 4 - 1
        offset = n % 7 - 3
        target = n % 9
        answer = value_at(initial, coefficient, offset, target)
        pair_id = f"rewrite-{pair:03d}" if n < 50 else None
        coefficient_term = "a_n" if coefficient == 1 else f"{coefficient}a_n"
        offset_term = f" + {offset}" if offset >= 0 else f" - {abs(offset)}"
        relation = f"{coefficient_term}{offset_term}"
        add(
            i,
            "valid_affine",
            f"Given a_0 = {initial} and a_(n+1) = {relation}, evaluate the recurrence at n = {target}.",
            answer,
            True,
            pair_id,
        )
        i += 1
        if pair_id:
            shifted_answer = value_at(initial, coefficient, offset, target)
            add(
                i,
                "valid_affine_rewrite",
                f"The sequence has a_1 = {initial} and a_(n+1) = {relation}; evaluate its term at n = {target + 1}.",
                shifted_answer,
                True,
                pair_id,
            )
            i += 1
            pair += 1

    for n in range(60):
        add(
            i,
            "missing_initial_condition",
            f"The recurrence a_(n+1) = {n % 3 + 1}a_n + 2 is evaluated at n = {n % 8 + 2}.",
        )
        i += 1
    for n in range(40):
        add(
            i,
            "ambiguous_indexing",
            f"Given a_0 = {n % 5} and a_(n+1) = 2a_n + 1, evaluate the recurrence.",
        )
        i += 1
    for n in range(40):
        add(
            i,
            "unsupported_nonlinear",
            f"Given a_0 = {n % 5 + 1} and a_(n+1) = a_n^2 + 1, evaluate at n = {n % 5 + 2}.",
        )
        i += 1
    for n in range(40):
        add(
            i,
            "unsupported_higher_order",
            f"Given a_0 = 1, a_1 = 2, and a_(n+2) = a_(n+1) + a_n, evaluate at n = {n % 6 + 2}.",
        )
        i += 1
    for n in range(40):
        add(
            i,
            "conflicting_definition",
            f"Given a_0 = {n % 4} and a_0 = {n % 4 + 1}, with a_(n+1) = 2a_n + 1, evaluate at n = 3.",
        )
        i += 1
    for n in range(130):
        add(
            i,
            "malformed_recurrence",
            f"Evaluate the recurrence a_0 = {n % 3} at n = 4; no recurrence rule is supplied.",
        )
        i += 1

    assert len(cases) == 500, len(cases)
    output = Path("data/recurrence_ood_v1.json")
    output.write_text(
        json.dumps(
            {
                "schema_version": 1,
                "oracle": "python-recurrence-descriptor-v1",
                "cases": cases,
            },
            indent=2,
        )
        + "\n"
    )
    print(f"wrote {len(cases)} cases to {output}")


if __name__ == "__main__":
    main()
