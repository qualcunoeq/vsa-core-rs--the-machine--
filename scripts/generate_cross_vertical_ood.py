"""Independent corpus for the cross-vertical composition benchmark."""
import json
from pathlib import Path

cases = []

def add(i, family, s1, s1_out, s2, s2_in, s2_out, expected, auth=True, pair=None, tamper=False):
    cases.append({
        "id": f"cv.{i:04d}", "family": family, "stage_one": s1,
        "stage_one_output": s1_out, "stage_two": s2,
        "stage_two_input": s2_in, "stage_two_output": s2_out,
        "expected": expected, "should_authorize": auth,
        "pair_id": pair, "tamper_intermediate": tamper,
    })

i = 0
for n in range(100):
    a0, p, q, target, addend = 2 + n % 8, 1 + n % 3, n % 4, 2 + n % 4, 1 + n % 7
    value = a0
    for _ in range(target): value = p * value + q
    add(i, "recurrence_algebra", f"Given a_0 = {a0} and a_(n+1) = {p}*a_n + {q}, find a_n at n = {target}", "Integer", f"Evaluate {{intermediate}} + {addend}", "Integer", "Integer", str(value + addend)); i += 1

for n in range(100):
    x, y, p, q, target = 2 + n % 9, 1 + n % 6, 1 + n % 2, n % 3, 2 + n % 4
    seed = x + y
    value = seed
    for _ in range(target): value = p * value + q
    add(i, "algebra_recurrence", f"Evaluate {x} + {y}", "Integer", f"Given a_0 = {{intermediate}} and a_(n+1) = {p}*a_n + {q}, find a_n at n = {target}", "Integer", "Integer", str(value)); i += 1

for n in range(80):
    a0, p, q, target = 2 * (1 + n % 7), 1 + n % 2, 2 * (n % 3), 1 + n % 4
    value = a0
    for _ in range(target): value = p * value + q
    rhs = value + 4
    x, y = (rhs + 2) // 2, (rhs - 2) // 2
    add(i, "recurrence_system", f"Given a_0 = {a0} and a_(n+1) = {p}*a_n + {q}, find a_n at n = {target}", "Integer", "Solve system: x + y = {intermediate} + 4; x - y = 2 for x,y", "Integer", "SolutionSet", '{"x": "%d", "y": "%d"}' % (x, y)); i += 1

for n in range(10):
    a0, p, q, target = 2 + n, 2, 1, 2
    value = p * (p * a0 + q) + q
    pair = f"rewrite-{n}"
    add(i, "recurrence_algebra", f"Given a_0 = {a0} and a_(n+1) = {p}*a_n + {q}, find a_n at n = {target}", "Integer", "Evaluate {intermediate} + 5", "Integer", "Integer", str(value + 5), pair=pair); i += 1
    add(i, "recurrence_algebra", f"For the sequence with a_0={a0}, a_(n+1)={p}a_n+{q}, find the term at n={target}", "Integer", "Compute {intermediate} + 5", "Integer", "Integer", str(value + 5), pair=pair); i += 1

for n in range(20):
    a0, p, q, target = 2 + n % 5, 2, 1, 2
    add(i, "forged_intermediate", f"Given a_0 = {a0} and a_(n+1) = {p}*a_n + {q}, find a_n at n = {target}", "Integer", "Evaluate {intermediate} + 1", "Integer", "Integer", None, False, tamper=True); i += 1
for n in range(10):
    add(i, "incompatible_handoff", "Given a_0 = 2 and a_(n+1) = 2*a_n + 1, find a_n at n = 2", "Integer", "Solve system: x + y = 5; x - y = 1 for x,y", "SolutionSet", "SolutionSet", None, False); i += 1
for n in range(10):
    add(i, "unsupported_stage", f"Given a_0 = 2 and a_(n+1) = a_n^2 + 1, find a_n at n = {n + 2}", "Integer", "Evaluate {intermediate} + 1", "Integer", "Integer", None, False); i += 1

assert len(cases) == 340, len(cases)
out = {"schema_version": 1, "oracle": "independent integer recurrence/algebra/system composition oracle v1", "cases": cases}
Path("data/cross_vertical_ood_v1.json").write_text(json.dumps(out, indent=2) + "\n")
print(f"wrote {len(cases)} cases")
