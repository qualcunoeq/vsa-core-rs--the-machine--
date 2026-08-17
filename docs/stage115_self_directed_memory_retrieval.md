# Stage 115 — Self-directed memory retrieval

The self-directed planner consumed typed gaps and admitted only artifacts with
exact version, artifact, source-catalog provenance, and prerequisite closure.
It remained proposal-only and refused stale versions, unknown artifacts, and
unavailable sources.

| Metric | Result |
|---|---:|
| Gap observations | 1,200 |
| Complete plans | 600 |
| Stale-version refusals | 200 |
| Unknown-artifact refusals | 200 |
| Unavailable-source refusals | 200 |
| Plan replay | 1,200/1,200 |
| Plan tamper rejection | 1,200/1,200 |
| Source catalogs | 3 |
| Provenance mismatches | 0 |
| Prerequisite failures | 0 |
| Manifest mutation | 0 |
| Live route mutation | 0 |

The planner creates no executable capability and does not promote source
records; it only produces replayable, auditable retrieval proposals.
