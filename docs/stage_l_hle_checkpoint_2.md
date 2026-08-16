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
  `e4eab7e9e0bb5abb940b123297166fefee30c9494f1357906bf5d8f3c618ab86`
* Producer commit: `8d5e026`
* Trace SHA-256:
  `49dc1eb48c0ee065fc70a034d7e61f0e7333f7b7661e035530a8fd04e421df26`
* Summary SHA-256:
  `d78e2d52186fcb4acb9b976cfb78b8b55554ccc48f371a81e530702fab8d08e0`

The checkpoint is diagnostic only. Its unchanged score is evidence that
foundational curriculum growth has not yet crossed the HLE technical-language
and specialist-method boundary, while the zero-false-authorization property
remains intact.
