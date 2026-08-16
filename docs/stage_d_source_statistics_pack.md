# Stage D — Source-derived finite statistics

The first previously absent domain acquired through the generic source-formula
catalog runtime is finite descriptive statistics. The records are attributed
to OpenStax *Introductory Statistics 2e* and carry explicit input constraints;
the executor has no statistics-specific formula branches.

The catalog covers arithmetic and weighted means, Bernoulli variance, and the
mean and variance of a finite binomial model. The independent 240-case corpus
contains 120 supported, 40 ambiguous, and 80 refused cases, including missing
inputs, zero weights, invalid probabilities, and unsupported domains.

Results: 240/240 exact decisions, 120/120 supported values, 120/120 source
records preserved, 240/240 replay verification, 240/240 tamper rejection, and
zero false authorizations or denials. Corpus hash:
`576f8a76f7273e3bf0a265fd2cf1330ebc80b2b315f961d9b5116d8e4656427f`.

Run:

```text
cargo run --quiet --bin source_statistics_pack_bench
```
