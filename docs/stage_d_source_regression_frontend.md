# Stage D — Source-derived regression technical frontend

The new regression catalog is reachable through a bounded language frontend
that accepts explicit operation words and labeled rational quantities. It
never infers a design matrix, a statistical model, or a theorem assumption
from generic words such as “fit” or “association.” Successful frontend output
is still only a typed request; the generic source catalog must replay and
execute it separately.

## Results

| Measure | Result |
|---|---:|
| Cases | 240 |
| Supported / ambiguous / unsupported / missing | 120 / 40 / 40 / 40 |
| Exact frontend decisions | 240/240 |
| Supported values after pack execution | 120/120 |
| Frontend replay | 240/240 |
| Pack replay on invoked requests | 120/120 |
| Frontend tamper rejection | 240/240 |
| Pack tamper rejection | 120/120 |
| Pack invocations | 120 |
| False authorizations / denials | 0 / 0 |

The machine-readable report is
[`stage_d_source_regression_frontend.json`](stage_d_source_regression_frontend.json).

This frontend remains shadow-only and does not alter the live router or HLE
holdout. Reproduce it with:

```text
cargo run --quiet --bin source_regression_frontend_bench
```
