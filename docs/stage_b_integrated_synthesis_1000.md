# Stage-B integrated synthesis checkpoint

This is a shadow-only, independently authored 1,000-case corpus exercising
five routes assembled from independently validated curriculum capabilities:

1. finite topology → strict specialization graph → adjacency matrix → linear algebra;
2. finite graph + exact probability → one-step random walk;
3. DNA base composition → uniform-position finite distribution;
4. molecular formula → semantically labelled element-count vector → linear algebra;
5. exact combinatorial count → explicit number-theory operand → Bézout certificate.

Each route contains 140 supported, 30 ambiguous, and 30 refused cases. The
route receipt records the terminal authorization, intermediate-artifact count,
replay status, tamper result, and first failure gate. Ambiguous and refused
routes are never authorized merely because an upstream artifact exists.

## Results

The machine-readable report is
[`stage_b_integrated_synthesis_1000.json`](stage_b_integrated_synthesis_1000.json)
with corpus hash
`781eceaf31fc44519cddd67b2ac62858815482488f851ba6726564402d96c42a`.

| Metric | Result |
|---|---:|
| Cases | 1,000 |
| Supported / ambiguous / refused | 700 / 150 / 150 |
| Exact terminal decisions | 1,000 / 1,000 |
| Supported routes authorized | 700 / 700 |
| Emitted intermediate entries | 2,560 |
| Case-level replay verification | 1,000 / 1,000 |
| Tamper rejections | 1,000 / 1,000 |
| Failure gates localized | 300 / 300 |
| False authorizations | 0 |
| False denials | 0 |
| Route leakage | 0 |

The five route families each contain 200 cases. The topology route preserves
the explicit strict-specialization policy and vertex order. The random-walk
route requires an explicit row-stochastic convention and rejects multi-step
requests. The biology route requires the `uniform_position` sampling policy.
The chemistry route preserves the element basis and rejects an unlabelled
stoichiometric interpretation. The combinatorics route declares the count's
number-theoretic role before constructing a Bézout certificate.

No live registry, executor, or curriculum manifest is mutated by this run.
