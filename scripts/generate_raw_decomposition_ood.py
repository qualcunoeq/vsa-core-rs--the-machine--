import json
from pathlib import Path

cases = []
def add(i, prompt, sig, decompose=True):
    cases.append({"id": f"raw.{i:04d}", "prompt": prompt, "expected_signature": sig, "should_decompose": decompose})

i = 0
for n in range(150):
    add(i, f"Compute {1+n%9} + {2+n%7}", "None>Integer"); i += 1
for n in range(150):
    a, p, q, t, k = 2+n%8, 1+n%3, n%4, 1+n%4, 2+n%6
    add(i, f"Starting with a_0 = {a} and a_(n+1) = {p}*a_n + {q}; find a_n at n = {t}, then evaluate a_n + {k}.", "None>Integer/Some(Integer)>Integer"); i += 1
for n in range(100):
    a, p, q, t = 2*(1+n%7), 1+n%2, 2*(n%3), 1+n%4
    add(i, f"Given a_0 = {a} and a_(n+1) = {p}*a_n + {q}; find a_n at n = {t}, then solve system: x + y = a_n + 4; x - y = 2.", "None>Integer/Some(Integer)>SolutionSet"); i += 1
for n in range(50):
    a, p, q, t, k, tail = 2+n%6, 2, 1, 1+n%3, 3+n%5, 4+n%4
    add(i, f"Given a_0 = {a} and a_(n+1) = {p}*a_n + {q}; find a_n at n = {t}, then add {k} and use that value as a_0 in a_(n+1) = 1*a_n + {tail} at n = 2.", "None>Integer/Some(Integer)>Integer/Some(Integer)>Integer"); i += 1
for n in range(25):
    add(i, f"Either compute 2 + {n+3} directly or use a staged route.", None, False); i += 1
for n in range(25):
    add(i, f"The recurrence is under-specified and has no requested index: a_(n+1) = 2*a_n + 1.", None, False); i += 1

assert len(cases) == 500
Path("data/raw_decomposition_ood_v1.json").write_text(json.dumps({"schema_version": 1, "oracle": "independent raw decomposition structure oracle v1", "cases": cases}, indent=2) + "\n")
print(f"wrote {len(cases)} cases")
