# Stage 92 — sealed transfer for source-derived interpolation

Stage 92 measures transfer of the newly acquired linear-interpolation domain
on an independently authored permanent partition.  It is linked to, but does
not mutate, the historical 5,000-case curriculum examination.  The new corpus
contains 360 development, 120 validation, and 120 sealed holdout reports.

| Measure | Result |
|---|---:|
| Total cases | 600 |
| Supported / ambiguous / unsupported | 360 / 120 / 120 |
| Supported authorized | 360/360 |
| Ambiguities preserved | 120/120 |
| Unsupported refused | 120/120 |
| Replay verification | 600/600 |
| Tamper rejection | 600/600 |
| Provenance preserved | 600/600 |
| False authorizations / denials | 0 / 0 |
| Manifest mutation | false |

The sealed partition independently contains 72 supported, 24 ambiguous, and 24
unsupported reports, all replay-verified.  The report records the source
catalog hash, prior sealed-checkpoint hash, question hashes, and complete
receipts in `stage92_interpolation_sealed_transfer.json`.

Reproduction:

```text
RUSTFLAGS='-Awarnings' cargo run --quiet --bin stage92_interpolation_sealed_transfer
```

This is a transfer checkpoint, not an HLE result.  Production routing and the
curriculum manifest remain unchanged.
