# Stage-K sealed curriculum examination

This checkpoint evaluates integrated technical-language transfer over 5,000
reports and ten bounded domains: finite topology, chemistry, DNA biology,
complex arithmetic, finite statistics, combinatorics, elementary number theory,
calculus, scalar ODEs, and finite graph theory.

The corpus is permanently partitioned before execution:

* 3,000 development reports;
* 1,000 validation reports;
* 1,000 sealed holdout reports.

The executor receives only report text. Hidden supported/ambiguous/unsupported
labels are retained by the scorer and are not passed into routing. Each
authorization requires a typed downstream artifact, provenance, replay, and a
tamper rejection. The run does not mutate the curriculum manifest or live
registry.

## Results

The machine-readable report is
[`stage_k_sealed_curriculum_exam_5000.json`](stage_k_sealed_curriculum_exam_5000.json).

Producer commit for this checkpoint: `48e3567`.

| Metric | Result |
|---|---:|
| Total reports | 5,000 |
| Supported / ambiguous / unsupported | 3,000 / 1,000 / 1,000 |
| Supported authorizations | 3,000 / 3,000 |
| Ambiguities preserved | 1,000 / 1,000 |
| Unsupported reports refused | 1,000 / 1,000 |
| Replay verification | 5,000 / 5,000 |
| Tamper rejection | 5,000 / 5,000 |
| Provenance preserved | 5,000 / 5,000 |
| False authorizations | 0 |
| False denials | 0 |

The sealed partition contains 600 supported, 200 ambiguous, and 200
unsupported reports. Its question hash is
`cfdc8930bd0da69f448a5d6cc7e1a813f3a6c346c63f245bc37a3522263fbae8`.
The full question corpus hash is
`b99c6afcfb1fa72b02de1ff17b5f58ed1df52584897ab28aa2c172bdcf94c29f`.

This is a bounded curriculum exam, not an uncontrolled HLE or open-world
evaluation. It establishes a larger permanent holdout and a repeatable
learning-curve instrument for later curriculum expansion.
