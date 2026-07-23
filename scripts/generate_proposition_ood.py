#!/usr/bin/env python3
"""Generate an implementation-independent proposition proof corpus.

The oracle here is deliberately tiny and independent of the Rust kernel: it
only labels whether each descriptor describes a valid or invalid proof shape.
The Rust benchmark interprets the descriptor into a Proof object and checks
that the kernel agrees with this oracle.
"""

import json
from pathlib import Path


FAMILIES = [
    ("refl", True),
    ("add_zero", True),
    ("mul_one", True),
    ("commutativity", True),
    ("distributivity", True),
    ("symmetry", True),
    ("transitivity", True),
    ("abs_nonnegative", True),
    ("forall_intro", True),
    ("missing_premise", False),
    ("wrong_hypothesis", False),
    ("distractor_premise", False),
    ("wrong_certificate", False),
    ("unknown_theorem", False),
    ("converse_confusion", False),
    ("wrong_expected", False),
]


def main() -> None:
    cases = []
    for i in range(500):
        # Adjacent records form a semantic-preserving pair and therefore use
        # the same proof family and oracle label.
        family, expected = FAMILIES[(i // 2) % len(FAMILIES)]
        value = (42 + i * 19) % 17 - 8
        pair_id = None
        if i >= 16:
            pair_id = f"rewrite-{(i - 16) // 2:03d}"
        cases.append(
            {
                "id": f"prop-ood-{i:04d}",
                "family": family,
                "value": value,
                "rewrite": i % 2 == 1,
                "pair_id": pair_id,
                "should_accept": expected,
            }
        )
    corpus = {"schema_version": 1, "oracle": "python-descriptor-v1", "cases": cases}
    out = Path("data/propositions_ood_v1.json")
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(corpus, indent=2) + "\n")
    print(f"wrote {len(cases)} cases to {out}")


if __name__ == "__main__":
    main()
