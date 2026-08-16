# Stage A: bounded combinatorics curriculum pack

The Machine now has a shadow-only, exact finite-counting substrate. The pack
is deliberately narrower than general combinatorics and refuses unbounded,
analytic, or representation-dependent claims.

## Contract

Supported operations are bounded permutations, combinations, multinomial
counts, two-set inclusion--exclusion, the pigeonhole minimum, Stirling numbers
of the second kind, and surjection counts. Inputs are integer parameters with
explicit scope and provenance; results are replayable exact scalar artifacts.

The pack rejects invalid domains, oversized requests, inconsistent set counts,
missing parameters, and unresolved interpretations such as labeled versus
unlabeled selection. It does not infer a probability model, graph semantics,
or number-theoretic meaning from a count alone.

## Independent validation

The frozen corpus is recorded in
`docs/stage_a_combinatorics_pack.json` with SHA-256
`236249ed13e59b509fab31829e3c61a2caebeff38cefce46352c610fa042eb12`.

| outcome | cases |
| --- | ---: |
| supported | 120 |
| ambiguous | 40 |
| refused | 80 |
| exact decisions | 240/240 |
| supported artifacts | 120/120 |
| replay verified | 240/240 |
| tamper rejected | 240/240 |
| false authorizations | 0 |
| false denials | 0 |

This is a curriculum artifact, not a live registry entry. It is marked
`shadow_validated` in the breadth-first manifest and remains subject to later
cross-domain composition and promotion gates.
