# Stage 126 — simplicial-homology technical-language frontend

The new homology capability now accepts a controlled technical-language
surface with explicit operation, vertex, simplex, and coefficient declarations.
The frontend supports bounded paraphrase structure while refusing inferred
fields, persistent/infinite/continuous variants, integer coefficients, and
unsupported homology semantics.

| Measure | Result |
|---|---:|
| Cases | 240 |
| Supported / ambiguous / unsupported | 120 / 40 / 80 |
| Exact frontend decisions | 240/240 |
| Frontend replay verified | 240/240 |
| Frontend tamper rejected | 240/240 |
| Downstream authorized | 120/120 |
| Downstream replay verified | 120/120 |
| Downstream tamper rejected | 120/120 |
| False authorizations / denials | 0 / 0 |

The corpus is independent of the preconstructed homology requests.  Every
authorized result passes through textual parsing, typed lowering, bounded
homology execution, and replay; ambiguity and unsupported cases never enter
execution.

Reproduce with:

```text
cargo run --quiet --bin stage126_simplicial_language_frontend
```

Machine-readable report: `docs/stage126_simplicial_language_frontend.json`.
