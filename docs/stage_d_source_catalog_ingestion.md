# Stage D — generic source-catalog ingestion

Finite-statistics source records are now loaded from a cited JSON catalog and
validated by a domain-agnostic ingestion routine before execution. The
validator checks unique identifiers and aliases, expression inputs, constraint
references, and citation completeness. It does not know the meaning of any
formula or contain statistics-specific branches.

| metric | result |
| --- | ---: |
| catalog records | 5 |
| valid catalogs accepted | 1/1 |
| mutated catalogs tested | 5 |
| mutated catalogs rejected | 5/5 |
| generic constraint-generated exercises | 5 |
| generated exercises complete and replayed | 5/5 |
| deterministic catalog replay | true |
| false acceptances | 0 |

Catalog hash: `970987600d185d310e52862dc36a8551c35cb53cfe0b8f550e591a5abe2f0be0`.

The execution pack remains shadow-only. A valid catalog is not sufficient for
promotion or live routing; independent exercises, boundary tests, replay, and
policy authorization remain required.

Run:

```text
cargo run --quiet --bin source_catalog_ingestion_bench
```
