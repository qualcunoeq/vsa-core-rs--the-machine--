#!/usr/bin/env python3
"""Generate an implementation-independent 2x2 systems OOD corpus.

The expected solutions are computed with Python's Fraction arithmetic rather
than by importing the Rust executor.  Prompts, equation order, signs, and
failure classes are deliberately varied so the corpus is not a copy of the
development generator.
"""

from fractions import Fraction
import json
from pathlib import Path


OUT = Path(__file__).resolve().parents[1] / "data" / "algebra_systems_ood_v2.json"


def equation(a, b, c):
    def term(coef, var):
        sign = "+" if coef >= 0 else "-"
        magnitude = abs(coef)
        return f"{sign}{magnitude}*{var}"

    return f"{a}*x{term(b, 'y')}={c}"


def solution(a, b, c, d, e, f):
    determinant = a * e - b * d
    if determinant == 0:
        return None
    x = Fraction(c * e - b * f, determinant)
    y = Fraction(a * f - c * d, determinant)
    return {"x": str(x), "y": str(y)}


def case(case_id, prompt, expected, authorize):
    return {
        "id": case_id,
        "tier": "independent-v2",
        "method": "linear_system",
        "prompt": prompt,
        "expected_result": json.dumps(expected, separators=(",", ":")) if expected else None,
        "should_authorize": authorize,
    }


def main():
    cases = []
    variants = []
    forms = [
        lambda q, r: f"Solve system: {q}; {r} for x,y",
        lambda q, r: f"Use {q} and {r} to determine x,y.",
        lambda q, r: f"Find x,y from {q}; {r}.",
        lambda q, r: f"Solve simultaneously: {q} and {r}, for x and y.",
        lambda q, r: f"The pair obeys {q} and {r}. Solve for x,y.",
    ]
    variant_forms = [
        lambda q, r: f"For x,y, solve the simultaneous equations {r} and {q}.",
        lambda q, r: f"Solve the pair {r}; {q} for x,y.",
        lambda q, r: f"Determine x and y from {r} together with {q}.",
        lambda q, r: f"Given {r} and {q}, find the ordered pair x,y.",
    ]

    # 120 unique systems: coefficients and target values are generated from
    # independent integer schedules, then solved exactly with Fraction.
    for i in range(120):
        a = 1 + (i * 7) % 8
        b = -4 + (i * 11) % 9
        d = -5 + (i * 13) % 11
        e = 1 + (i * 17) % 8
        if a * e == b * d:
            e += 1
        x = -6 + (i * 19) % 13
        y = -5 + (i * 23) % 11
        c, f = a * x + b * y, d * x + e * y
        q, r = equation(a, b, c), equation(d, e, f)
        expected = solution(a, b, c, d, e, f)
        base_id = f"sysv2-unique-{i:03}"
        cases.append(case(base_id, forms[i % len(forms)](q, r), expected, True))
        variants.append({
            "id": base_id + "-v",
            "base_id": base_id,
            "case": case(base_id + "-v", variant_forms[i % len(variant_forms)](q, r), expected, True),
        })

    # 30 inconsistent systems: proportional left sides, incompatible RHS.
    for i in range(30):
        a, b = 1 + i % 6, 2 + (i * 3) % 5
        q = equation(a, b, 3 + i)
        r = equation(2 * a, 2 * b, 9 + i)
        base_id = f"sysv2-inconsistent-{i:03}"
        cases.append(case(base_id, forms[i % len(forms)](q, r), None, False))
        variants.append({
            "id": base_id + "-v",
            "base_id": base_id,
            "case": case(base_id + "-v", variant_forms[i % len(variant_forms)](q, r), None, False),
        })

    # 20 dependent systems: proportional equations with compatible RHS.
    for i in range(20):
        a, b, c = 1 + i % 5, 1 + (i * 2) % 6, 2 + i
        q = equation(a, b, c)
        r = equation(3 * a, 3 * b, 3 * c)
        base_id = f"sysv2-dependent-{i:03}"
        cases.append(case(base_id, forms[i % len(forms)](q, r), None, False))
        variants.append({
            "id": base_id + "-v",
            "base_id": base_id,
            "case": case(base_id + "-v", variant_forms[i % len(variant_forms)](q, r), None, False),
        })

    # 20 nonlinear systems, including products and squares. These must remain
    # unsupported rather than being coerced into the linear backend.
    for i in range(20):
        q = f"x*y={2 + i}"
        r = f"x+y={5 + i}"
        base_id = f"sysv2-nonlinear-{i:03}"
        cases.append(case(base_id, forms[i % len(forms)](q, r), None, False))
        variants.append({
            "id": base_id + "-v",
            "base_id": base_id,
            "case": case(base_id + "-v", f"Solve the nonlinear pair {q} and {r}.", None, False),
        })

    # 10 malformed/ambiguous prompts: too many equations or no unique target.
    for i in range(10):
        q = equation(1, 1, 2 + i)
        r = equation(1, -1, i)
        base_id = f"sysv2-malformed-{i:03}"
        prompt = f"Solve {q}, {r}, and x=4 for x,y."
        cases.append(case(base_id, prompt, None, False))
        variants.append({
            "id": base_id + "-v",
            "base_id": base_id,
            "case": case(base_id + "-v", f"Can x,y be determined from {q} and {r}?", None, False),
        })

    payload = {"schema_version": 1, "cases": cases, "variants": variants}
    OUT.write_text(json.dumps(payload, indent=2) + "\n")
    print(f"wrote {OUT} ({len(cases)} base, {len(variants)} variants)")


if __name__ == "__main__":
    main()
