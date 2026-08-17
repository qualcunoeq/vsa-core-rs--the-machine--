# Stage 154 — Clone-only curriculum admission for visual source routes

Stage 153 established registry-level promotion and rollback for the raw-OCR
science routes. This stage exercises the curriculum boundary that follows it:

```text
validated source report
→ validated promotion report
→ prerequisite closure
→ curriculum-pack proposal
→ cloned-manifest validation
→ admission or fail-closed rejection
```

The production curriculum manifest is never modified. Every proposal is
validated in a cloned manifest, replayed deterministically, and checked against
a tampered policy hash.

## Results

The Stage 152 source report and Stage 153 promotion report both pass preflight
for all `240` scenarios. The admission corpus contains `80` clean proposals
and `160` deliberately invalid proposals: missing prerequisite, duplicate
boundary, incomplete promotable gates, or an unfrozen HLE policy.

Results are `240/240` exact admission decisions, `80` admissions, `160`
fail-closed blocks, `240/240` deterministic replays, `240/240` tamper
rejections, `200/240` prerequisite-closure checks (the remaining 40 are the
intentional missing-prerequisite cases), zero false admissions, zero false
rejections, and zero live-manifest mutations.

Machine-readable receipts are in
`docs/stage154_visual_source_curriculum_admission.json`.
