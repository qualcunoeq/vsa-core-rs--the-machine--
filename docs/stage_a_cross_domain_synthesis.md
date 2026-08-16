# Stage A/B: independent cross-domain synthesis corpus

The first 1,000-case synthesis campaign exercises generic whitelisted method
specifications over routes spanning two to five validated domains. Route
execution is shadow-only; a synthesized spec cannot call an untrusted domain,
exceed depth eight, or mutate a registry. Ambiguous semantic bindings stop
before execution, and unsupported domains fail static validation.

The corpus is recorded in `docs/stage_a_cross_domain_synthesis.json` with
SHA-256 `26c3d79c175e3cdbe4d56c2046b5b1fc36ae1009fcd2f2733fcda1b45e644e60`.

| metric | result |
| --- | ---: |
| cases | 1,000 |
| supported routes | 950/950 |
| ambiguous cases | 25/25 |
| unsupported cases | 25/25 |
| exact decisions | 1,000/1,000 |
| failure localization | 1,000/1,000 |
| replay verified | 1,000/1,000 |
| tamper rejected | 1,000/1,000 |
| valid synthesized specs | 975 |
| budget-compliant specs | 1,000/1,000 |
| false authorizations | 0 |
| immutable parents | 1,000/1,000 |

Routes cover combinatorics with finite probability, graph and linear-algebra
representations, continuous ODE/calculus/mechanics composition, elementary
number theory with bounded dynamics, and a five-domain aggregate audit. This
is a synthesis and routing gate, not a claim of broad unrestricted expertise.
