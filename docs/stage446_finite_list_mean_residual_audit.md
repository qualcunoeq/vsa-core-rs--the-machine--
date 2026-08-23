# Stage 446 — finite-list-mean residual audit

This answer-key-blind audit examines the existing `FiniteListMean` route on
the 3,000-question development partition. It does not change the frontend,
read sealed data, ingest sources, or mutate production state.

| Metric | Result |
|---|---:|
| Questions in dataset / development read | 4000 / 3000 |
| Complete / ambiguous / unsupported / missing | 10 / 21 / 32 / 2937 |
| Complete execution / shadow authorizations | 10 / 10 |
| Frontend replay / tamper rejection | 3000 / 3000 |
| Execution replay / tamper rejection | 10 / 10 |
| Answer keys / sealed questions read | 0 / 0 |
| Source ingestion / production mutation | 0 / 0 |

The residual mechanisms are heterogeneous rather than one obvious safe
extension:

* 7 ambiguous prompts contain a mean target but no explicit finite list;
* 14 ambiguous prompts do not expose a finite list in the current textual
  boundary;
* 27 unsupported prompts require ranges, filtering, symbolic, or optimization
  semantics;
* 4 require symbolic or word-valued list entries;
* 1 requests a derived rate/change rather than a finite-list mean.

The ten complete cases are already the finite-list candidates counted in the
development-only Stage 445 portfolio score. The residual audit therefore does
not justify broadening this route yet; the next useful work is targeted
representation analysis (tables/graphs, ranges, and symbolic lists) with
independent corpora for each mechanism.

Evidence hashes:

* dataset SHA-256: `3cf924116a0f8f6a0c84d0ce7949b0c1e16221e0d4b5fcb0c4322110e30714f2`
* frontend source SHA-256: `b595be14a50da0e92af3957591c299c02982911644a10ab6bd5b32dec2b575bb`
* report SHA-256: `b77727576f48ea217f67eb2294b5b028b3342eeceb04f59016ab79ed6d53eb58`

Reproduce with:

```text
STAGE446_REPORT_JSON=/tmp/stage446.json \\
STAGE446_REPORT_MD=/tmp/stage446.md \\
cargo run --quiet --bin stage446_finite_list_mean_residual_audit
```
