# Stage 153 — Governed promotion and rollback for visual/source routes

This campaign exercises the complete lifecycle of the validated raw-OCR
science routes from Stage 152 without mutating the live registry:

```text
validated shadow candidate
→ dependency and policy preflight
→ cloned-registry staging
→ promotion or fail-closed rejection
→ later counterexample
→ rollback
→ historical replay
```

The source report is preflighted before any candidate is staged. Its hash is
recorded in the machine-readable report, and every scenario carries a
deterministic promotion receipt. Tampered receipts are rejected by fingerprint
mismatch. Later-counterexample scenarios mutate only the cloned registry and
verify that accumulated world-state hashes and historical replay remain intact
after rollback.

## Results

The independent Stage 152 source/multimodal report was accepted as the input
artifact (`600/600` exact decisions, `0` false authorizations, `0` false
denials). Stage 153 evaluates `240` lifecycle cases:

| Scenario | Cases | Expected outcome |
|---|---:|---|
| clean promotion | 60 | promoted |
| regression blocked | 40 | blocked |
| dependency conflict | 40 | rejected |
| migration failure | 30 | rejected |
| later counterexample | 40 | promoted then rolled back |
| competing boundary | 30 | rejected |

All `240/240` promotion decisions, receipt replays, and tamper rejections
match. The campaign records `100` promotions, `140` blocked/rejected
proposals, `40` detected later regressions, and `40` successful rollbacks with
world-state preservation and historical replay. False authorizations and
false denials are both zero. The production registry remains unchanged; all
mutations occur in cloned registries.

The machine-readable receipt set is
`docs/stage153_visual_source_promotion.json`.
