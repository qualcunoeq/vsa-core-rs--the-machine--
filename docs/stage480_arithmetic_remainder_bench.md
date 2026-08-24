# Stage 480 — bounded arithmetic-remainder pressure benchmark

This independent corpus validates an arithmetic-expression remainder bridge
separate from the frozen literal-remainder route. The accepted grammar is
bounded integer literals with grouping and `+`, `-`, `*`, and `^`; variables,
division, factorials, implicit multiplication, polynomial remainders, and
oversized powers remain refused.

| Metric | Result |
|---|---:|
| Cases | **240** |
| Supported / ambiguous / unsupported | **120 / 40 / 80** |
| Exact decisions | **240/240** |
| Supported values | **120/120** |
| Frontend replay | **240/240** |
| Execution replay | **120/120** |
| Tamper rejections | **360/360** |
| False authorizations / denials | **0 / 0** |

The original Stage 468 literal-remainder benchmark remains unchanged at
240/240 after the new operation was added. The separate operation preserves
the old route boundary rather than silently widening it.

Evidence:

* corpus SHA-256: `e9f63fe4790a3ab1aef34aa4f7c955937e2af5ca7b91bb85b6c495de49c1d005`
* report SHA-256: `6381f496eda0d5e05db008e2c5665d1cf3243ee1528c173796ace5ef46714a58`
* serialized report file SHA-256: `cd74e9803ef4161734d35c1403877905000a0a042a317f090071fc410384dd03`

