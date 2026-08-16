# Stage D — Source-derived finite statistics

The first previously absent domain acquired through the generic source-formula
catalog runtime is finite descriptive statistics. The records are loaded from
the cited data artifact
`docs/sources/openstax_finite_statistics_catalog.json`, attributed to OpenStax
*Introductory Statistics 2e*, and carry explicit input constraints; the
executor has no statistics-specific formula branches.

The catalog covers arithmetic and weighted means, Bernoulli variance, and the
mean and variance of a finite binomial model. The independent 240-case corpus
contains 120 supported, 40 ambiguous, and 80 refused cases, including missing
inputs, zero weights, invalid probabilities, and unsupported domains.

Results: 240/240 exact decisions, 120/120 supported values, 120/120 source
records preserved, 240/240 replay verification, 240/240 tamper rejection, and
zero false authorizations or denials. Corpus hash:
`5011f87ac0de00147ca87bf66c348324aa81d1b4c113e81e6d279a89473cae04`.

The catalog ingestion gate separately accepts the five-record artifact and
rejects five mutated catalogs (duplicate IDs, undeclared inputs or
constraints, duplicate aliases, and incomplete citations).

Run:

```text
cargo run --quiet --bin source_statistics_pack_bench
```
