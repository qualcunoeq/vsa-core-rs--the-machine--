# Stage L — Post-curriculum HLE checkpoint 2

This is a frozen diagnostic evaluation after the sealed 5,000-case curriculum
exam. It does not modify the router, curriculum manifest, production registry,
or authorization policy. Curriculum routes remain shadow-only.

## Reproduction

```text
cargo run --quiet --bin stage_l_hle_checkpoint_2
```

The run reads only `data/hle.jsonl`, invokes the existing
`QuestionRouter::orchestrate` path, and writes the per-question trace to
`/tmp/hle_curriculum_checkpoint_2.jsonl`. The committed summary is
`stage_l_hle_checkpoint_2.json`.

| Field | Value |
|---|---:|
| Questions | 2,500 |
| Correct authorized answers | 2 |
| Incorrect authorized answers | 0 |
| False authorizations | 0 |
| Curriculum signals | 608 |
| Pack invocations | 0 |
| Native replay receipts | 0 |
| Compatibility replay verified | 2 |
| Replay not applicable | 2,498 |
| Replay not recorded | 0 |
| Registry mutation | false (shadow-only) |

The two historical authorized answers do not carry native plan-execution
receipts. The checkpoint therefore performs a deterministic compatibility rerun
for each and records `compatibility_verified` only when the answer is identical.
This is explicitly distinct from native receipt replay; it avoids silently
counting missing receipt coverage as verified.

## Terminal classification

| Classification | Cases |
|---|---:|
| Correct authorized answer | 2 |
| Visual input required | 260 |
| No curriculum signal | 1,694 |
| Language-normalization failure | 63 |
| Missing factual knowledge | 386 |
| Missing reasoning method | 80 |
| Unsupported target | 9 |
| Ambiguous or unresolved | 6 |

No curriculum pack reached execution. The result therefore does not claim HLE
transfer from the newly validated curriculum; it identifies the remaining
language, specialist-knowledge, target, and visual-input boundaries without
loosening authorization.

## Immutable inputs and hashes

* Dataset SHA-256:
  `31b26cc8e352af16bedb9a714feb788e562be38898ab92dc54b4665882bf1c6`
* Curriculum-manifest SHA-256:
  `c99fea9200db643958fad69e017ed8879f958345f2fc509973e9ecc266c015cd`
* Producer commit: `e3fb629`
* Trace SHA-256:
  `e912f176fb9f37ed885b3471bd3c8fc9a7abfebbe2ccea688deb03e3ede4e69d`
* Summary SHA-256:
  `0ecb90a390dbc8b963b55d22a2b4cc45f90c1f89d2fd669b692f3f599a46e546`

The checkpoint is diagnostic only. Its unchanged score is evidence that
foundational curriculum growth has not yet crossed the HLE technical-language
and specialist-method boundary, while the zero-false-authorization property
remains intact.
