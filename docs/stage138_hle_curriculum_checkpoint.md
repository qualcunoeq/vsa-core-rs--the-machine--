# Stage 138 — post-curriculum HLE checkpoint

This frozen diagnostic run evaluates the current release candidate after the
arithmetic-functions pack, its cross-domain composition, and route-blind
technical-language integration. The HLE dataset and answers remain untouched
during development; new curriculum routes remain shadow-only.

| Measure | Result |
|---|---:|
| Questions | 2,500 |
| Correct authorized answers | 2 |
| Incorrect authorized answers / false authorization | 0 / 0 |
| Curriculum signals | 643 |
| Pack invocations | 0 |
| Compatibility replay verified | 2 |
| Replay not applicable | 2,498 |
| Replay not recorded | 0 |
| Registry mutation | false |
| Total execution time | 49,833,151 µs |
| Maximum question time | 540,626 µs |

The terminal distribution was 2 correct authorizations, 260 visual-required
questions, 1,668 without a curriculum signal, and 570 unresolved signals. The
zero-invocation result is a transfer diagnosis: the validated shadow packs
still have no production-language handoff, rather than evidence that they
should be routed by vocabulary.

Dataset SHA-256: `31b26cc8e352af16bedb9a714feb788ae562be38898ab92dc54b4665882bf1c6`

Manifest SHA-256: `e252a0d7e1632815efde3dd5d6044e4e4aa3b9d697485b215e4269450943cb31`

Trace SHA-256: `efc57a256c44be3c836ebc65005d2e79f10f3340a6fbf4193ec763baac6e2e4f`

The complete per-question trace is recorded in
`docs/stage138_hle_curriculum_checkpoint.trace.jsonl`; it contains question
and reference hashes, route traces, answer provenance, replay classification,
and execution timing without storing answer-key text.

Reproduce with:

```text
cargo run --quiet --bin stage138_hle_curriculum_checkpoint
```
