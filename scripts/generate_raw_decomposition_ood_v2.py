import json
from pathlib import Path

cases = []
def add(i, prompt, sig, should=True):
    cases.append({"id": f"rawv2.{i:04d}", "prompt": prompt, "expected_signature": sig, "should_decompose": should})

i = 0
direct_forms = [
    "Please calculate {a} plus {b}.", "What is {a} + {b}?", "Find the sum of {a} and {b}.",
    "Compute {a} + {b}.", "Ignoring the irrelevant quantity 99, evaluate {a} plus {b}.",
]
for n in range(300):
    a, b = 1 + n % 17, 2 + n % 13
    add(i, direct_forms[n % len(direct_forms)].format(a=a, b=b), "None>Integer"); i += 1

for n in range(250):
    a, p, q, t, k = 2+n%8, 1+n%3, n%4, 1+n%4, 2+n%6
    forms = [
        f"For a sequence, a_0 = {a}; a_(n+1) = {p}*a_n + {q}. Find the term at n = {t}; afterwards calculate a_n + {k}.",
        f"Let a_0={a}. Each next term is {p} times a_n plus {q}. Determine the term at n={t}; then add {k}.",
        f"The sequence has a_0 = {a} and a_(n+1) = {p}a_n + {q}. Find a_(n) for the term {t}, then evaluate a_n + {k}.",
        f"Given a_0={a} and a[n+1]={p}*a[n]+{q}, find the term at n={t}; afterwards add {k}.",
    ]
    add(i, forms[n % len(forms)], "None>Integer/Some(Integer)>Integer"); i += 1

for n in range(200):
    a, p, q, t = 2*(1+n%7), 1+n%2, 2*(n%3), 1+n%4
    forms = [
        f"Given a_0 = {a} and a_(n+1) = {p}*a_n + {q}, find a_n at n = {t}, then solve system: x + y = a_n + 4; x - y = 2.",
        f"The sequence starts at a_0={a}; each next term is {p} times a_n plus {q}. At term {t}, solve system: x+y=a_n+4; x-y=2.",
    ]
    add(i, forms[n % 2], "None>Integer/Some(Integer)>SolutionSet"); i += 1

for n in range(100):
    a, p, q, t, k, tail = 2+n%6, 2, 1, 1+n%3, 3+n%5, 4+n%4
    add(i, f"For a_0={a}, a_(n+1)={p}*a_n+{q}, find the term at n={t}; afterwards add {k} and use that value as a_0 in a_(n+1)=1*a_n+{tail} at n=2.", "None>Integer/Some(Integer)>Integer/Some(Integer)>Integer"); i += 1

for n in range(75):
    add(i, f"Either compute {n+2} + {n+3} directly, or use the sequence route.", None, False); i += 1
for n in range(75):
    add(i, f"The sequence is described by a_(n+1) = 2*a_n + 1, but its initial value and requested index are omitted; ignore the unrelated number {n}.", None, False); i += 1

assert len(cases) == 1000, len(cases)
Path("data/raw_decomposition_ood_v2.json").write_text(json.dumps({"schema_version": 1, "oracle": "independent textbook-style decomposition oracle v2", "cases": cases}, indent=2) + "\n")
print(f"wrote {len(cases)} cases")
