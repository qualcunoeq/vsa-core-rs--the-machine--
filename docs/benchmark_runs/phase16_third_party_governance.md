# Phase 16 — third-party corpus governance

The project now has a separate release format for genuinely independent
evaluation data.  It records:

- source citation, locator, license, retrieval date, and content SHA-256;
- original prompt text (never normalized before evaluation);
- explicit scope labels: `in_scope`, `understandable_unsupported`,
  `ambiguous`, and `outside_scope`;
- source item identifiers and development/locked-holdout splits;
- a deterministic release hash.

Validation rejects missing provenance, unknown source references, scope/oracle
mismatches, duplicate IDs, invalid source hashes, and releases without both
development and holdout cases.  The evaluator delegates execution to the
existing governed decomposition path; it does not infer annotations or mutate
capabilities.

The current checked-in fixture is intentionally **not evidence**:

```bash
cargo run --release --quiet --bin third_party_corpus_bench \
  data/third_party_corpus_fixture_v1.json
```

It exists only to test schema and release-hash behavior.  It is marked
`release_kind = fixture` and explicitly says it is not third-party evidence.
No third-party prompts have been fabricated or silently represented as sourced.

The next acquisition step is therefore operational rather than algorithmic:
populate a new `release_kind = third_party` manifest from at least four
independent sources, preserve the original wording, obtain independent scope
and oracle annotations, and freeze a permanent holdout before running the
Machine.
