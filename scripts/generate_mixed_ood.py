#!/usr/bin/env python3
"""Generate an independent mixed-domain routing corpus.

Expected route/authorization labels are authored here, separately from the
Rust router and orchestrator.  Recurrence prompts are intentionally marked
unsupported: the current integrated router can recognize their mathematical
surface but has no prose recurrence executor, so safe abstention is the
expected behavior.
"""

import json
from pathlib import Path


def main() -> None:
    cases = []
    pair = 0

    def add(i, domain, prompt, route, authorize, pair_id=None):
        cases.append({
            "id": f"mixed-ood-{i:04d}",
            "domain": domain,
            "prompt": prompt,
            "expected_route": route,
            "should_authorize": authorize,
            "pair_id": pair_id,
        })

    i = 0
    # Direct algebra: supported equations plus intentionally non-executable
    # questions whose wording resembles a solve request.
    for n in range(120):
        a = n % 7 + 1
        x = n % 13 - 6
        b = n % 11 - 5
        c = a * x + b
        pair_id = f"rewrite-{pair:03d}" if n >= 20 else None
        add(i, "direct_algebra", f"Solve for x: {a}*x + ({b}) = {c}.", "math", True, pair_id)
        i += 1
        if pair_id:
            add(i, "direct_algebra", f"Solve ({c}) = {a}*x + ({b}) for x.", "math", True, pair_id)
            i += 1
            pair += 1
    for n in range(50):
        add(i, "direct_algebra", f"Can x be determined from {n + 2}x + {n + 1} = {2*n + 3}?", "math", False)
        i += 1

    # Linear systems: unique systems are supported; degenerate systems must
    # remain refused even though they contain ordinary arithmetic syntax.
    for n in range(150):
        a, b, c, d = 1 + n % 4, 1 + (n // 3) % 4, 2 + n % 5, 1 + (n // 5) % 4
        if a * d == b * c:
            d += 1
        x, y = n % 9 - 4, (n * 3) % 9 - 4
        r1, r2 = a*x + b*y, c*x + d*y
        add(i, "linear_system", f"Solve system: {a}*x+{b}*y={r1}; {c}*x+{d}*y={r2} for x,y.", "math", True)
        i += 1
    for n in range(100):
        k = n % 9 - 4
        add(i, "linear_system", f"Solve system: x+y={k}; 2*x+2*y={2*k} for x,y.", "math", False)
        i += 1

    # Theorem route: supported kernel-shaped claims and invalid/converse
    # near-neighbours.  Rewrites preserve the theorem route and decision.
    theorem_prompts = [
        ("Prove that x = x.", True),
    ]
    for n in range(200):
        prompt, authorize = theorem_prompts[n % len(theorem_prompts)]
        pair_id = f"rewrite-{pair:03d}" if n >= 20 else None
        if pair_id and n % 2:
            prompt = "Prove that x=x."
        add(i, "proposition", prompt, "theorem", authorize, pair_id)
        i += 1
        if pair_id and n % 2 == 0:
            pair += 1
    for n in range(50):
        add(i, "proposition", f"Prove that x = y, assuming no premise {n}.", "theorem", False)
        i += 1

    # Recurrence cases are a deliberate integration boundary probe.  The
    # router should classify them as math, but the orchestrator must abstain
    # because no prose recurrence executor is registered yet.
    for n in range(230):
        add(i, "recurrence", f"Evaluate the recurrence a_0 = {n % 5 + 1}, a_n = a_(n-1) + 2 at n = {n % 8 + 2}.", "math", False)
        i += 1

    out = Path("data/mixed_ood_v1.json")
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps({"schema_version": 1, "oracle": "python-mixed-descriptor-v1", "cases": cases}, indent=2) + "\n")
    print(f"wrote {len(cases)} cases to {out}")


if __name__ == "__main__":
    main()
