# Conversation evaluation (Phase 9)

Earlier phases made the system observable and its knowledge inspectable.
Phase 9 makes the whole application part of its own evaluation: representative
conversation traces become versioned regression cases, an untouched evaluation
set is kept separate, and every mechanism is measured as a paired ablation.

The guiding rule: **measure the complete application, including its failures.**
A wrong delivered answer and a system abstention are different outcomes and are
never added together.

## Corpus

Two versioned corpora under `data/`:

* `conversation_eval_v1.json` — the frozen regression set. Cases are reviewed
  and never edited once frozen; the trace captured from them gates releases.
* `conversation_eval_holdout_v1.json` — the untouched evaluation set. It is
  reported but never gated, so it stays honest.

Each case is a conversation (a sequence of steps) with gold labels per step:
whether the turn should be answered, answer substrings that must and must not
appear, whether a clarification is expected, whether the turn depends on context,
how many earlier turns a correction should invalidate, and any pinned
capability.

## Metrics

| Metric | What it reveals |
|---|---|
| Answer correctness | Whether the delivered answer is right (oracle, against gold) |
| Answer coverage | How often it can answer at all |
| Unsupported assertions | Whether it claims more than the evidence supports |
| Clarification success | Whether questions resolve ambiguity |
| Context accuracy | Whether follow-ups reference the right information |
| Correction propagation | Whether updates affect subsequent answers |
| Latency | Whether the system is practical (mean / p50 / p95 / max) |
| Memory | The conversation-state footprint against the declared budget |

Two rules keep the numbers from flattering the system:

* **Oracle scoring is separate from system self-rejection.** A delivered answer
  is judged right or wrong against gold; an abstention or clarification is the
  system's own decision and is counted separately. The report shows both, so a
  system that answers little and is never wrong cannot look correct.
* **Unsupported assertions** count only delivered *answers to questions* with no
  evidence. Teaching acknowledgements and clarifications are not assertions. The
  declared limit is zero.

## Traces

A run is snapshotted to a trace (`TurnTrace` per turn: outcome, answer text,
request kind, capability, evidence count, verification, follow-up kind/source,
latency, knowledge counts). `detect_drift` compares two traces field by field,
normalizing freshly generated provenance ids and latency so only behavioural
change counts. `Trace::digest` is the stable hash of the normalized trace, so
`capture` and `replay` can agree on integrity without being defeated by random
ids.

## Paired ablations

`ConversationService::EvalConfig` adds six toggles that all default to
production behaviour:

| Mechanism | Enabled | Disabled |
|---|---|---|
| `vsa_retrieval` | reconstruction-energy retrieval (`answer_all`) | lexical exact-term retrieval |
| `context_retrieval` | rank reference candidates with session hypervector memory | exact referents only |
| `typed_capabilities` | typed solver / unit-conversion adapter | general question path |
| `reuse` | accept reused / associated retrieved claims | direct matches only |
| `semantic_worker` | shadow worker offers a stored reading as a clarification | plain abstention |
| `consolidation` | feed the turn back into the bounded working context | reconstruct from stored turns |

Each mechanism is disabled in turn while the *same* corpus runs; the report
contains both metric sets and the signed delta. No baseline is hard-coded.

## Result

Measured by `cargo run --bin machine_eval` on the committed corpora:

* regression: 28 turns, 18 answered, oracle 18 / 18 correct, 0 unsupported
  assertions, 0 drift;
* holdout: 13 turns, 10 answered, oracle 10 / 10 correct, 0 unsupported;
* six paired ablations, all preserving safety.

The deltas are informative rather than decorative. Disabling
`typed_capabilities` raises the oracle-wrong rate (the solver path is load
bearing); disabling `consolidation` drops correction propagation to zero;
disabling `context_retrieval` or `reuse` raises abstentions and lowers
clarification success; disabling `semantic_worker` turns its clarifications back
into abstentions.

The binary exits non-zero if the regression trace drifts, if any run delivers an
unsupported assertion, or if an ablation fails to preserve safety — so a release
can be assessed automatically using representative conversations, resource
limits, and frozen evaluation cases.
