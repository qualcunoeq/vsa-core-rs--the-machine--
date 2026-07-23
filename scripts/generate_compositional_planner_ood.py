import json
from pathlib import Path

tasks = []

def step(inp, out, prompt, cost, support):
    return {"input": inp, "output": out, "prompt": prompt, "cost": cost, "support": support}

def add(i, candidates, expected, auth=True):
    tasks.append({"id": f"plan.{i:04d}", "candidates": candidates, "expected": expected, "should_authorize": auth})

i = 0
for n in range(140):
    a, b = 1 + n % 9, 2 + n % 7
    add(i, [{"id": "direct", "steps": [step(None, "Integer", f"Evaluate {a} + {b}", 1, 100)]}], str(a + b)); i += 1

for n in range(150):
    a0, p, q, target, addend = 2 + n % 8, 1 + n % 3, n % 4, 1 + n % 4, 2 + n % 6
    value = a0
    for _ in range(target): value = p * value + q
    valid = [step(None, "Integer", f"Given a_0 = {a0} and a_(n+1) = {p}*a_n + {q}, find a_n at n = {target}", 2, 80), step("Integer", "Integer", f"Evaluate {{intermediate}} + {addend}", 1, 80)]
    invalid = [step(None, "Integer", "Evaluate recurrence result", 1, 100)]
    add(i, [{"id": "fresh-composition", "steps": valid}, {"id": "direct-invalid", "steps": invalid}], str(value + addend)); i += 1

for n in range(100):
    a0, p, q, target = 2 * (1 + n % 7), 1 + n % 2, 2 * (n % 3), 1 + n % 4
    value = a0
    for _ in range(target): value = p * value + q
    rhs = value + 4
    x, y = (rhs + 2) // 2, (rhs - 2) // 2
    valid = [step(None, "Integer", f"Given a_0 = {a0} and a_(n+1) = {p}*a_n + {q}, find a_n at n = {target}", 2, 70), step("Integer", "SolutionSet", "Solve system: x + y = {intermediate} + 4; x - y = 2 for x,y", 2, 70)]
    invalid = [step(None, "SolutionSet", "Solve system: x*y = 2; x + y = 3 for x,y", 1, 100)]
    add(i, [{"id": "system-composition", "steps": valid}, {"id": "nonlinear-direct", "steps": invalid}], '{"x": "%d", "y": "%d"}' % (x, y)); i += 1

for n in range(50):
    a0, p, q, target, addend, tail = 2 + n % 6, 2, 1, 1 + n % 3, 3 + n % 5, 4 + n % 4
    value = a0
    for _ in range(target): value = p * value + q
    middle = value + addend
    final = middle
    for _ in range(2): final = 1 * final + tail
    chain = [step(None, "Integer", f"Given a_0 = {a0} and a_(n+1) = {p}*a_n + {q}, find a_n at n = {target}", 2, 90), step("Integer", "Integer", f"Evaluate {{intermediate}} + {addend}", 1, 90), step("Integer", "Integer", f"Given a_0 = {{intermediate}} and a_(n+1) = 1*a_n + {tail}, find a_n at n = 2", 2, 90)]
    add(i, [{"id": "three-stage", "steps": chain}], str(final)); i += 1

for n in range(25):
    a, b = n + 2, n + 5
    direct = [step(None, "Integer", f"Evaluate {a} + {b}", 3, 10)]
    composed = [step(None, "Integer", f"Evaluate {a} + {b}", 1, 100), step("Integer", "Integer", "Evaluate {intermediate} + 0", 1, 100)]
    add(i, [{"id": "direct-expensive", "steps": direct}, {"id": "composed-cheap", "steps": composed}], str(a + b)); i += 1

for n in range(25):
    add(i, [{"id": "bad-handoff", "steps": [step(None, "Integer", "Evaluate 2 + 3", 1, 100), step("SolutionSet", "Integer", "Evaluate {intermediate} + 1", 1, 100)]}, {"id": "unsupported", "steps": [step(None, "Integer", "Evaluate unsupported expression", 1, 1)]}], None, False); i += 1

for n in range(10):
    add(i, [{"id": "tie-a", "steps": [step(None, "Integer", "Evaluate 2 + 3", 1, 50)]}, {"id": "tie-b", "steps": [step(None, "Integer", "Evaluate 2 + 4", 1, 50)]}], None, False); i += 1

assert len(tasks) == 500, len(tasks)
Path("data/compositional_planner_ood_v1.json").write_text(json.dumps({"schema_version": 1, "oracle": "independent planner route oracle v1", "cases": tasks}, indent=2) + "\n")
print(f"wrote {len(tasks)} tasks")
