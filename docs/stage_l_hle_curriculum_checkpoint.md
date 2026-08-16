# Stage L: current curriculum HLE checkpoint

The current curriculum checkpoint ran the unchanged 2,500-question HLE
dataset under commit `828f74f` with the curriculum manifest hash recorded in
the JSON report. It remained shadow-only and did not mutate routing, packs, or
the HLE corpus.

Results: 2 correct authorized answers, 0 incorrect authorizations, 705
curriculum signals, 0 pack invocations, and 0 false authorizations. The
dominant first failures were missing factual prerequisites (451), visual
dependencies (260), and no curriculum signal (1,614). The two historical
authorized answers remain replay-not-recorded in this checkpoint, matching the
existing release-accounting limitation; no replay failure occurred.

The full per-question trace was written to `/tmp` and its hash is preserved in
`stage_l_hle_curriculum_checkpoint.json`.
