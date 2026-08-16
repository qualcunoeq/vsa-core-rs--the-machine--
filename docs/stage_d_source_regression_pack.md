# Stage D — Source-derived finite regression diagnostics

This is the first source-derived domain added in this campaign rather than a
new subject-specific evaluator. Five regression relations are represented as
records in an attributed source transcription and executed by the existing
domain-agnostic rational formula runtime.

The source-to-execution path is:

```text
attributed source transcription
→ generic formula extraction and catalog validation
→ independent exercise generation
→ exact formula execution
→ boundary and mutation rejection
```

The catalog covers a bounded linear-regression scope: slope, intercept,
fitted value, residual, and coefficient of determination. It refuses missing
inputs, zero variation, unsupported domains, ambiguous formulations, and
unknown claims. It does not implement statistical estimation, uncertainty
intervals, significance tests, or an evaluator branch keyed to a formula name.

## Results

| Measure | Result |
|---|---:|
| Cases | 240 |
| Supported / ambiguous / refused | 120 / 40 / 80 |
| Exact decisions | 240/240 |
| Supported values | 120/120 |
| Source provenance preserved | 120/120 |
| Replay verified | 240/240 |
| Tamper rejected | 240/240 |
| Source-catalog mutations rejected | 6/6 |
| Generic generated exercises complete | 5/5 |
| False authorizations / denials | 0 / 0 |

The machine-readable report is
[`stage_d_source_regression_pack.json`](stage_d_source_regression_pack.json).
The source transcription is
[`openstax_finite_regression_source.txt`](sources/openstax_finite_regression_source.txt).

This remains shadow-only. The live router, registry, and frozen HLE holdout
are unchanged.

Reproduce with:

```text
cargo run --quiet --bin source_regression_pack_bench
```
