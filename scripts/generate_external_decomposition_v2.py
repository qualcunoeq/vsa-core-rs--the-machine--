"""Create a fresh, untouched external-style holdout after v1 hardening."""

import json
from pathlib import Path

cases = []


def add(i, prompt, outcome, signature=None):
    cases.append(
        {
            "id": f"external-v2.{i:04d}",
            "source": "fresh-hand-audited-holdout",
            "split": "development" if i < 80 else "holdout",
            "prompt": prompt,
            "expected_outcome": outcome,
            "expected_signature": signature,
        }
    )


i = 0
for n in range(60):
    a, b = 3 + n % 29, 5 + n % 19
    add(i, f"Determine the total when {a} is added to {b}.", "supported", "None>Integer")
    i += 1
for n in range(60):
    a, b = 4 + n % 23, 6 + n % 17
    add(i, f"Work out the result obtained by combining {a} and {b} by addition.", "supported", "None>Integer")
    i += 1
for n in range(40):
    a, b = 2 + n % 31, 7 + n % 13
    add(i, f"What number results if {a} is increased by {b}?", "supported", "None>Integer")
    i += 1
for n in range(20):
    add(i, f"Please calculate {8 + n} plus {3 + n % 11}.", "supported", "None>Integer")
    i += 1
for n in range(10):
    a, p, q, t, k = 3 + n % 5, 1 + n % 3, n % 4, 1 + n % 3, 2 + n % 5
    add(
        i,
        f"Let a_0={a}. Each next term is {p} times a_n plus {q}. Determine the term at n={t}; then add {k}.",
        "supported",
        "None>Integer/Some(Integer)>Integer",
    )
    i += 1
for n in range(5):
    add(i, f"Either determine the total when {n + 4} is added to {n + 6}, or use another route.", "ambiguous")
    i += 1
for n in range(5):
    add(i, f"The recurrence a_(n+1)=2*a_n+1 is given, but no initial value or target index is supplied; ignore {n}.", "unsupported")
    i += 1

assert len(cases) == 200
assert sum(case["split"] == "holdout" for case in cases) == 120
Path("data/external_decomposition_v2.json").write_text(
    json.dumps(
        {
            "schema_version": 1,
            "oracle": "fresh untouched hand-audited external-style holdout v2",
            "holdout_locked": True,
            "cases": cases,
        },
        indent=2,
    )
    + "\n"
)
print("wrote 200 cases (development=80, holdout=120)")
