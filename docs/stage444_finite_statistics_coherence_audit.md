# Stage 444 — finite-statistics coherence audit

This answer-key-blind audit applies the existing `FiniteStatistics` frontend
to the 3,000-question development partition of the external portfolio. It
does not broaden the frontend, read answer keys or sealed questions, ingest a
source, or mutate the registry or curriculum manifest.

## Result

| Metric | Result |
|---|---:|
| Questions in dataset / development read | 4000 / 3000 |
| Complete / ambiguous / unsupported / missing | 0 / 61 / 7 / 2932 |
| Complete execution cases | 0 |
| Frontend replay / tamper rejection | 3000 / 3000 |
| Execution replay / tamper rejection | 0 / 0 |
| Shadow authorizations | 0 |
| Answer keys / sealed questions read | 0 / 0 |
| Source ingestion / promotion proposals | 0 / 0 |
| Production or registry mutations | 0 |

The 61 ambiguous cases are dominated by a single lexical boundary:

* 59 report that a mean/average is present but do not provide the labeled
  inputs required by the source catalog;
* 2 mention a binomial quantity without identifying the requested output.

Running the already-existing finite-list-mean route over those 61 cases gives
10 complete, 21 ambiguous, 2 missing, and 28 unsupported. This is diagnostic
evidence that the residuals cross several distinct problem forms (explicit
lists, weighted/table data, rates, extrema, and specialist mathematics), not
evidence for one new statistics capability.

## Interpretation

The current source-statistics route has no complete development candidate and
therefore no execution or HLE-transfer claim. The correct next action is not to
loosen the route or infer missing quantities. The adjacent finite-list route
already handles the ten complete cases; the remaining residuals require
separate semantics or remain unsupported.

The machine-readable run records all 61 ambiguous case IDs, exact reason
counts, paired-route counts, replay/tamper results, and mutation flags.

Reproduce with:

```text
STAGE444_REPORT_JSON=/tmp/stage444.json \\
STAGE444_REPORT_MD=/tmp/stage444.md \\
cargo run --quiet --bin stage444_finite_statistics_coherence_audit
```

Evidence hashes:

* dataset SHA-256: `3cf924116a0f8f6a0c84d0ce7949b0c1e16221e0d4b5fcb0c4322110e30714f2`
* frontend source SHA-256: `b595be14a50da0e92af3957591c299c02982911644a10ab6bd5b32dec2b575bb`
* report SHA-256: `f191b3633fb3036abb6d70b941f6ab142ba197c1efc7217b1994a90ee4700780`
