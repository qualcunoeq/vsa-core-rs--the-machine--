# Development Environment

Recorded 2026-09-18 on `shibamobile`. This document is the reproducible setup
and startup reference for local development.

## 1. Machine inventory

| Resource | Value |
|---|---|
| Host / OS | `shibamobile`, Arch Linux (rolling), kernel 7.1.5-arch1-2 |
| CPU | Intel Core i7-1365U (Raptor Lake-U), 10 cores / 12 threads, AVX2 + AVX-VNNI, **no AVX-512** |
| Memory | 15 GiB RAM, 4 GiB swap |
| Storage | 476 GiB NVMe (`/` and `/home`), ~426 GiB free |
| GPU | Intel Iris Xe (integrated) — **no NVIDIA GPU**, no `nvidia-smi`, no CUDA toolkit |
| Toolchain | `rustc` 1.97.1 / `cargo` 1.97.1 (Arch package `rust` 1:1.97.1-1); `rustup` is not installed |
| C toolchain | gcc/cc 16.1.1, make, cmake, pkg-config, perl |
| Python | system Python 3.14 (no `sympy`); repo-local `.venv` has Python 3.14.6 + sympy 1.14 |

`Cargo.lock` is lockfile format v4 (304 packages; Phase 2 added `axum` and
`futures-util`).

## 2. Toolchain pin

`rust-toolchain.toml` pins `1.97.1` with the default profile. On this machine
the system Arch package already matches, so the file is documentation; on any
machine with `rustup` it forces the exact compiler version.

## 3. Portable vs locally optimized builds

`.cargo/config.toml` no longer sets `-C target-cpu=native`. The default build
is portable and is what benchmark comparisons should use:

```bash
cargo build --release --locked --lib
```

For local-only, non-portable performance builds use the checked-in native
config:

```bash
cargo build --release --locked --lib --config .cargo/config.native.toml
```

Rationale: the previous config hardcoded `target-cpu=native` and its comment
claimed AVX-512 support; this CPU exposes AVX2/AVX-VNNI only. Binaries built
with native codegen must not be redistributed.

## 4. Dependencies for the CPU build

No system libraries need installing: bundled SQLite (`rusqlite`) and `ring`
compile from source with the gcc toolchain above. CUDA is optional and off by
default. The symbolic-CAS paths shell out to `python3`; the system interpreter
has no `sympy`, while `.venv/` does. To exercise CAS paths:

```bash
python3 -m venv .venv
.venv/bin/pip install sympy
PATH="$PWD/.venv/bin:$PATH" cargo test --lib ...
```

## 5. Build

```bash
cargo build --locked --lib -j 3        # ~8m20s from a clean target/, 9 warnings
```

Use `-j 3` or `-j 4`: a single `rustc` unit driving this crate peaks at
~4.3 GiB RSS and the machine has 15 GiB total.

The crate declares hundreds of `[[bin]]` targets and even more files under
`src/bin/` (565). Building all of them is not currently possible:
`stage243_second_source_corpus` and `stage173_route_blind_technical_language`
fail to compile (pre-existing, unrelated to the library). For that reason
`cargo build --bins` / bare `cargo test` are not part of the workflow.

## 6. Tests

```bash
cargo test --lib --locked -j 3          # library suite
cargo test --lib qa::tests              # QA engine only
cargo test -p conversation_runtime_integration --locked   # Phase 1 acceptance
```

Baseline recorded 2026-09-18 (clean portable debug build):

| Result | Count |
|---|---|
| passed | 2352 |
| failed | 42 (all pre-existing) |
| ignored | 15 |
| wall time | 388 s |

After the Phase 1 conversation module was added the suite reports
2357 passed / 42 failed / 15 ignored. After Phase 2 (6 new service tests and
5 new `chat_server` tests) a full run reports 2369 passed / 41 failed /
15 ignored. After Phase 3 (4 new service tests, 1 new `chat_server` test, and
8 new persistence tests) a full run reports 2383 passed / 42 failed /
15 ignored. After Phase 4 (24 new conversation tests, 2 new `chat_server`
tests, and 1 new `context` test) a full run on 2026-09-19 reports 2410
passed / 42 failed / 15 ignored in 644 s; no test outside the known
pre-existing failure set fails, and the stochastic pair swapped once again
(`indexer::test_learned_projector_outperforms_random_sampling` failed while
`predictive::tests::test_credit_assignment` passed). After Phase 5 (11
adapter tests, 5 service tests, 2 `chat_server` tests, and 1 frozen scenario
test) a full run on 2026-09-20 reports **2429 passed / 41 failed / 15
ignored** in 325 s; the 41 are the known pre-existing set (the stochastic pair
both passed this run, so the count is 42 − 1) and no test in the modules
touched by Phase 5 fails. After Phase 6 (9 `semantic_fidelity` tests and 7
`semantic_shadow` tests; the frozen coverage measurement is one of them) a full
run on 2026-09-20 reports **2445 passed / 41 failed / 15 ignored** in 340 s;
the failure set is identical to the Phase 5 run, and no module touched by
Phase 6 fails. After Phase 7 (13 `reliability` tests, 5 added `chat_server`
tests, and the report binary) a full run on 2026-09-27 reports **2463 passed /
41 failed / 15 ignored** in 356 s; the failure set is identical to the earlier
runs (the stochastic pair both passed, so the count is 42 − 1), and no module
touched by Phase 7 — `reliability`, `chat_server`, `lib.rs` memory paths,
`bin/machine_chat` — fails. After Phase 8 (7 `document_learning` tests, 4
`conversation` workflow tests, 1 `chat_server` test, 2 `persistence` tests, and
2 acceptance tests in the integration crate) a full run on 2026-09-27 reports
**2476 passed / 42 failed / 15 ignored** in 379 s; the failure set is identical
to the 41 known pre-existing failures plus the stochastic
`indexer::test_learned_projector_training_improves_over_block_sampling`, and no
module touched by Phase 8 — `document_learning`, `persistence::documents`,
`conversation` document methods, `bin/machine_docs`, `/api/documents` — fails.
After Phase 9 (6 `conversation_eval` tests plus 1 acceptance test in the
integration crate, and the `machine_eval` binary) a full run on 2026-09-27
reports **2483 passed / 41 failed / 15 ignored** in 351 s; the failure set is
identical to the known 41 pre-existing failures (the stochastic pair both
passed), and no module touched by Phase 9 — `conversation_eval`,
`conversation` evaluation toggles, `bin/machine_eval` — fails.
After Phase 10 (14 `autonomy_task` tests, 1 acceptance test, and the new
`reliability` guarantees) a full run on 2026-09-27 reports **2496 passed / 42
failed / 15 ignored** in 340 s; the failure set is identical to the known 41
pre-existing failures plus the stochastic
`indexer::test_learned_projector_outperforms_random_sampling`, and no module
touched by Phase 10 — `autonomy_task`, `reliability` guarantees, `bin/machine_eval`
`task` mode — fails.
After Phase 11 (14 `operator` tests, 1 acceptance test, the `machine` binary,
and the `G-RELEASE-SCHEMA` guarantee) a full run on 2026-09-28 reports **2511
passed / 41 failed / 15 ignored** in 318 s; the failure set is identical to the
known 41 pre-existing failures (the stochastic pair both passed, so the count is
42 − 1), and no module touched by Phase 11 — `operator`, `reliability`
guarantees, `bin/machine` — fails.

Failure breakdown for the 42 (38 fail regardless of environment):

| Category | Count | Examples | Cause |
|---|---|---|---|
| Missing external artifacts | 5 | `chess_learner::test_stockfish_*`, `pdf_reader::test_extract_text` | `./stockfish` binary and `data/openstax_pdfs/*.pdf` are not in the checkout |
| Curated-evidence gate | 11 | `qa::tests::test_answer_combined_single`, `router::tests::test_curated_*` | `answer_combined` only answers facts with audited `entailment_examples`; `store_fact` writes raw triples with none. The conversational runtime surfaces these as labelled retrieved claims instead (see `src/conversation/`) |
| Proof stack drift | 11 | `qa::tests::test_unification_*`, `test_theorem_prover_*`, `test_and_rule_*` | backward chaining over stored schematic rules no longer proves the historical cases |
| CAS / typed-math expectations | 8 (3 fixed by sympy, 5 stale) | `algebra::test_math_engine_second_derivative`, `tests::test_math_engine_arithmetic_via_qa` | tests expect the legacy sympy wire format while `safe_math_answer` now abstains unless the typed algebra island accepts the prompt |
| Formalization/capability drift | 6 | `capabilities::production_registry_contains_only_verified_function_v1`, `failure_taxonomy::*`, `formalization_benchmark::*`, `physics::test_verified_solve_problem` | behavioural drift between these modules and their test expectations |
| Stochastic flaky | 1-2 per run | `reason::tests::test_anchored_chain_contractivity`, `indexer::tests::test_learned_projector_outperforms_random_sampling`, `predictive::tests::test_credit_assignment` | all use `rand::thread_rng()`; each passes standalone most of the time, so the failure count stays at 42 while the names swap |

Exposing the repo-local `sympy` fixes `math::test_explicit_sympy_cas_directive`,
`router::test_latex_math_requires_a_complete_standalone_ast`, and
`router::test_typed_math_pipeline_solves_plain_prose_algebra_and_calculus`:

```bash
PATH="$PWD/.venv/bin:$PATH" cargo test --lib --locked -j 3 math::test_explicit_sympy_cas_directive
```

## 7. Chat interface (Phase 2)

The local web interface is a thin HTTP/SSE layer over the Phase 1 service
(`src/chat_server.rs` plus the embedded `web/` assets); no reasoning logic
lives in the server. Run it with:

```bash
cargo build --locked --bin machine_chat -j 3
cargo run --bin machine_chat -- --open        # http://127.0.0.1:8787
```

Options: `--host`, `--port`, `--db`, `--data-dir`, `--memory`, `--backup`,
`--restore`, `--open`. Durable state lives in the SQLite database
(`--db`, default `data/conversation/machine.db`). `--memory` and
`--data-dir` point at the legacy JSON snapshots (`qa_memory.json`,
`sessions.json`); those are imported exactly once into a fresh database and
never written again. See §8 for the storage model.

| Method | Path | Purpose |
|---|---|---|
| GET | `/` | single-page interface (embedded HTML/CSS/JS) |
| GET | `/api/health` | facts/rules/session counts + storage status |
| GET/POST | `/api/sessions` | list / create conversations |
| GET | `/api/sessions/{id}` | full history (stale marks included) |
| POST | `/api/sessions/{id}/turns` | `{text, mode: ask\|teach}`; SSE: `started`, `stage`, `turn`/`error` |
| GET | `/api/sessions/{id}/export?format=markdown\|json` | download |
| GET | `/api/turns/{turn_id}` | turn details |
| POST | `/api/turns/{turn_id}/cancel` | cooperative cancellation |
| GET | `/api/memory?q=&kind=&status=&limit=` | memory inspector snapshot with provenance |
| GET | `/api/memory/assertions` | list assertions (id/status/version/scope/source) |
| GET | `/api/memory/assertions/{id}` | one assertion |
| GET | `/api/memory/assertions/{id}/history` | version history |
| POST | `/api/memory/assertions/{id}/correct` | `{text, note?}`; marks dependent turns stale |
| POST | `/api/memory/assertions/{id}/retract` | `{reason?}`; tombstone + stale marks |
| POST | `/api/memory/assertions/{id}/forget` | `{confirm: true}`; deletes assertion and history |

Behaviour notes:

- Progress events are the real `TurnStage`s the service reports
  (`interpreting`, `retrieving_facts`/`reasoning`, `checking_result`,
  `updating_memory`, `rendering`); answers are delivered whole when complete.
- Cancellation is cooperative: the flag is checked between processing stages,
  and dropping the SSE stream (closing the tab) also cancels the turn.
- `mode: teach` also accepts `IF <subject verb object> THEN <subject verb
  object>` rules, so the Phase 1 causal-chain demonstration (proof-checked
  conclusion + `chain_replay` verification) is reachable from the browser.
- The memory panel shows every assertion with its provenance (source kind,
  session, turn, timestamp), version, and status, and offers Correct,
  Retract, Forget, and History actions. Correcting or retracting marks any
  answered turn that depended on the assertion as stale; a new question
  recomputes against current memory.
- Follow-up turns are grounded in conversation state (Phase 4): pronouns and
  demonstratives resolve to exact entities, corrections supersede the active
  fact, and "what if" / "explain" continue the last operation. Ambiguous
  references produce a clarification instead of a guessed binding; the next
  message can name the referent and the original request is re-run.
- Math and unit questions reach the typed, replay-verified capabilities
  (Phase 5): expression evaluation, single linear and quadratic equations,
  small linear systems, and explicit unit conversion. The turn reports the
  concrete capability (`structured_solver:<capability_id>`) and, on success,
  `verified` with the capability's replay method. The four failure modes are
  kept distinct in the outcome: uninterpretable wording and known-but-
  unsupported operations are `unsupported`, missing information is
  `clarification_needed`, and a result that fails its own verifier is
  `failed`. See §10.
- Server tests spin up a real listener on an ephemeral port:
  `cargo test --lib chat_server` (15 tests, including the Phase 7 auth and
  limit tests), `cargo test --lib conversation` (56 tests),
  `cargo test --lib persistence` (8 tests plus unrelated matches).
- **Binding and auth (Phase 7).** The server binds `127.0.0.1` by default. A
  non-loopback `--host` requires `--allow-remote` and a bearer token
  (`--token <value>` or `MACHINE_CHAT_TOKEN`). When a token is set, every
  `/api/*` request must send `Authorization: Bearer <token>`; static assets
  stay open. Request bodies, turn text length, concurrent in-flight turns, and
  per-turn execution time are all bounded (see §12).

## 8. Durable state (Phase 3)

Three kinds of state are kept apart in one SQLite database:

| State | Storage | Retention |
|---|---|---|
| Conversation history | `sessions` / `turns` tables | saved per session |
| Working context (topic, entities, pending question) | `sessions.working_context` (bounded JSON) | reconstructable from turns |
| Knowledge memory | `assertions` / `assertion_versions` / `sources` tables | persisted with provenance |

Design points:

- **Stable ids**: `session-`, `turn-`, `fact-`, `rule-`, `source-`, and
  `conclusion-` prefixed UUIDs, generated once and never reused.
- **Versioned records**: every teach, reaffirmation, correction, and
  retraction appends to `assertion_versions`; correcting replaces the
  payload and increments the version.
- **Transactional writes**: a turn, its dependency links, and the working
  context commit in one transaction; every knowledge operation is one
  transaction.
- **Schema versions and migrations**: `PRAGMA user_version` plus ordered
  migrations from `src/persistence/migrations/`. A database newer than the
  build is refused rather than misread.
- **Backup and restore**:
  `machine_chat --backup <file>` uses SQLite `VACUUM INTO` for a consistent
  snapshot; `machine_chat --restore <file>` validates integrity and schema
  version, then restores through SQLite's backup API and rebuilds in-memory
  state. `Database::backup_to` / `restore_from` are also available to the
  service (`ConversationService::backup` / `restore`).
- **Provenance**: every assertion carries a `sources` row (kind, session,
  turn, note, timestamp). Evidence items and memory changes expose the
  assertion id, and answered turns record which assertions they used.
- **Scope**: knowledge is shared (`scope = 'shared'`); conversation history
  and working context are per session. The interface labels this explicitly.
- **Invalidation**: correcting, retracting, or forgetting an assertion marks
  every dependent conclusion and turn stale with a reason. Retractions keep
  a tombstone row, and legacy JSON import is recorded in `meta` and never
  repeated, so retracted knowledge cannot be silently resurrected.
- **Explicit operations**: teach (turn in `teach` mode), correct, retract,
  and forget are available over HTTP and in the memory panel; retract and
  forget require confirmation.

Persistence tests (`src/persistence/tests.rs`) cover migrations, the
assertion lifecycle, stale marking, restart, import-once, working-context
bounds, and backup/restore. End-to-end restart/correction tests live in
`integration-tests/conversation_runtime` and the service/`chat_server` test
modules.

## 9. Follow-up conversation (Phase 4)

A turn is understood against the session's bounded working context before it
is classified. The context (`src/conversation/store.rs`) is convenience
state, never knowledge, and holds:

| Field | Purpose |
|---|---|
| `topic`, `entities`, `entity_refs` | current topic and the exact entities each turn introduced |
| `last_operation` | the last request and its arguments (input, equation, variable, solution, ...) |
| `last_result` | outcome, answer, provenance, and capability of the last turn |
| `assumptions` | temporary values from follow-ups (for example a new right-hand side) |
| `pending` | the clarification the runtime is still waiting on |
| `references` | bounded pointers to earlier turns |

`src/conversation/followup.rs` resolves references and recognizes follow-ups:

- **Reference resolution** works in tiers: exact entity match, unique entity
  of the right class (`person` vs `thing`, possessive forms attach to a
  following noun), then hypervector context similarity, then a clarification.
  The bounded Keep-Recent + Fold memory in `src/context.rs` is used only to
  rank candidates: each session memory stores entries labeled `turn:<id>` and
  `entity:<exact text>`; a candidate wins only if its turn's similarity is at
  least 0.55 and beats the runner-up by 0.03. Otherwise the runtime asks
  ("`"it"` could refer to the observatory or rates; which one do you mean?")
  and substitutes the chosen candidate back into the original utterance.
- **About an entity**: "What do you know about her?" summarizes the stored
  facts where the entity is the subject or object, with provenance.
- **Earlier state**: "Who managed it before?" reads the previous version of
  the active assertion from `assertion_versions` and reports the superseded
  value, not the current one.
- **Corrections**: "Actually, Bob manages it now." is recognized from its
  discourse/temporal markers, matched against exactly one active fact that
  differs in the subject or object slot, and applied through the same
  `correct_knowledge` path as the memory panel (new version + stale dependent
  turns). Several candidate facts produce a clarification; none produces a
  plain teach.
- **Solver follow-ups**: "Solve 2x + 3 = 11." is classified as a solver
  question (the typed algebra island now accepts a target inferred from a
  single-variable equation). "What if the right-hand side is 15?" substitutes
  the new value into the stored equation, re-runs the deterministic solver,
  and records the new value as a temporary assumption. "Explain the
  substitution." renders the algebra receipt (parsed/normalized equation,
  formula, replay check) for the last solve.
- **Session isolation**: working context and hypervector memory are per
  session; knowledge stays shared (Phase 3). References never leak across
  sessions, and a solver follow-up in a session with no equation asks for one.

Interfaces: every turn result carries
`interpretation.follow_up {kind, resolved, source, confidence}` (rendered in
the web details panel), and the pending clarification records its candidates
and the original utterance so a one-word reply can finish the request.

The frozen scenario set is
`integration-tests/conversation_runtime/scenarios/phase4.json`, run by
`frozen_phase4_scenarios` with `cargo test -p conversation_runtime_integration`.
It covers the observatory correction (including a restart between turns), the
solver sequence, topic change + hypervector resolution, ambiguous persons,
and session isolation. Unit and service tests live in
`src/conversation/followup.rs` and `src/conversation/service.rs`.

## 10. Connected capabilities (Phase 5)

Phase 5 makes a small set of mature mathematical capabilities reachable
through chat. Each one already had a grounded, independently replay-verified
backend; the new work is the adapter that reaches them consistently.

| Capability id | Backend | Input | Verifier |
|---|---|---|---|
| `expression_evaluation` | `src/expression_evaluation.rs` | grounded `Expression` + argument bindings | `replay_expression_evaluation` |
| `linear_equation_solve` | `src/linear_equation.rs` | grounded `Equation`, explicit target variable | `replay_linear_equation` |
| `quadratic_equation_solve` | `src/quadratic_equation.rs` | grounded `Equation`, explicit target variable | `replay_quadratic_equation` |
| `linear_system_solve` | `src/linear_system.rs` | explicit 2×2 system, unique solution | `replay_linear_system` |
| `unit_conversion` | `src/unit_aware_quantity.rs` + `src/unit_quantity_composition.rs` | explicit conversion with a stated factor | `compose_to_algebra` replay |

The adapter lives in `src/conversation/capability_adapter.rs` and follows the
same six steps for every capability:

1. **Interpret** the question (`interpret`), reusing the project's existing
   formalization and target builder rather than new regexes.
2. **Construct typed inputs** — a `FormalizedTarget` (or a unit artifact).
3. **Identify missing information or ambiguity** from the capability's own
   contract, not from a guess.
4. **Execute** through the capability's `execute_*` entry point.
5. **Verify** with that capability's `replay_*` checker.
6. **Render** the result with assumptions and evidence; the evidence item
   records `replay_verified: Some(true)` and the verification method.

`ConversationService::run_question` consults the adapter first (only for
math- and unit-shaped prompts, via `is_capability_shaped`) and falls back to
the library `QuestionRouter` for everything the adapter declines — CAS
calculus, physics, and factual QA are untouched. A successful turn records
`Capability::StructuredSolver { domain: <capability id> }`, so the interface
shows exactly which capability served the request.

**The four distinctions** are kept separate (`CapabilityDisposition` maps them
onto `TurnOutcome`):

| Situation | `CapabilityDisposition` | `TurnOutcome` |
|---|---|---|
| Result produced and replay passed | `Verified` | `Answered` |
| Understood the operation but do not support it (degenerate system, complex roots, unsupported unit family) | `OperationUnsupported` | `Unsupported { reason }` |
| Understood the operation, required information absent (unbound variable, no conversion factor) | `MissingInformation` | `ClarificationNeeded { question }` |
| Ran but failed its own verifier | `VerificationFailed` | `Failed { error }` |

A request that matches no capability returns `NotInterpreted`, and the caller
falls back; malformed math that neither the adapter nor the router can read is
reported honestly as `unsupported`, not answered.

The frozen scenario set is
`integration-tests/conversation_runtime/scenarios/phase5.json`, run by
`frozen_phase5_scenarios`; it covers the supported forms, paraphrases, missing
information, recognized-but-unsupported operations, malformed input, and
non-math wording. Adapter tests are in
`src/conversation/capability_adapter.rs`; end-to-end service and HTTP tests are
in `src/conversation/service.rs` and `src/chat_server.rs`.

## 11. Semantic fidelity and shadow worker connection (Phase 6)

Phases 1–5 answered *is this well formed?* (structural validation) and *can we
solve it?* (typed capabilities). Phase 6 adds the question the earlier phases
deliberately left open: **does the interpretation faithfully represent the
source?** A proposal can be structurally perfect — valid schema, valid evidence
spans, closed symbol scopes — and still reverse a relationship, flip a sign,
drop a condition, or invent an equation whose spans happen to be real
substrings.

The measurement lives in `src/semantic_fidelity.rs` and is **independent of**
`semantic_ir::validate_candidate`. Structural validity and semantic fidelity
are reported separately and never collapse into one number. The evaluator set
covers eight labelled categories:

| Category | What it catches |
|---|---|
| `correct` | the interpretation matches the source |
| `reversed_relationship` | "Alice has three more than Bob" read as Bob having more |
| `wrong_sign` | `+` read as `-` |
| `wrong_quantity` | a number that does not match the source |
| `missing_condition` | a condition in the source dropped from the interpretation |
| `invented_equation_valid_span` | an equation with a valid span that does not support it |
| `multiple_plausible` | one committed reading when several are plausible |
| `unsupported_domain` | a commitment in a domain with no labelled consumer |

`src/semantic_shadow.rs` connects stored worker proposals to the existing typed
consumer — decode, structural validation, then `semantic_handoff` lowering into
`EquationProblemBinding`, then `equation_classification` +
`route_classified_equation` — **in shadow mode**: nothing authorizes a
user-visible answer, every record carries `downstream_authorized = false`, and
the original worker configuration, raw output, validation diagnostics, and
replay mode are preserved. Interpretation accuracy and solver accuracy are
separate fields of `ShadowReport`.

Ambiguous proposals present a short, derivable clarification — the wording is
built from the candidate's own relation, e.g. `Do you mean that b = a + 3?` —
and never invented. A fail-closed `ShadowPolicy` maps each verdict to
`Proceed`, `Clarify`, or `Reject`; nothing but a faithful reading proceeds.

Stored-output replay (`ReplayMode::StoredOutputReplay`, a re-decode of recorded
bytes) is labelled separately from model regeneration
(`ReplayMode::ModelRegeneration`). The offline evaluator only ever uses the
former: no model endpoint, no answer keys.

If model-assisted answer wording is used, `guard_answer_wording` checks every
numeric and equation-shaped claim in the proposed text against the structured
result, so a displayed claim cannot outrun what was computed.

Frozen set and artifacts:

* gold corpus: `data/semantic_fidelity_gold_v1.json` (8 categories);
* coverage corpus: `data/semantic_fidelity_coverage_v1.json` (prose word
  problems the deterministic binder abstains on);
* evaluator: `cargo run --bin semantic_fidelity_eval` writes
  `docs/semantic_fidelity_eval_v1.jsonl`,
  `docs/semantic_fidelity_eval_v1.report.json`, and
  `docs/semantic_fidelity_eval_v1.md`.

**Complete when: the worker improves independently measured language coverage
while meeting an explicit wrong-answer limit.** Measured 2026-09-20 by the
offline evaluator:

| Metric | Value |
|---|---|
| interpretation accuracy | 1/8 (only the faithful case is labelled correct) |
| solver accuracy (faithful cases only) | 1/1 |
| wrong-answer limit | 0 |
| silent wrong answers | 0 |
| verdicts proceed / clarify / reject | 1 / 2 / 5 |
| coverage (baseline → worker) | 0/3 → 3/3 (lift 3) |
| downstream authorizations | 0 |
| replay mode | `stored_output_replay` |

The coverage lift is the independently measured language-coverage improvement
the phase asks for: on prose word problems with no explicit `=`, the
deterministic binder abstains (`0/3`) while the worker-assisted path reaches a
faithful, handoff-accepted interpretation (`3/3`). The explicit wrong-answer
limit is `0`, and the evaluator fails its process if any silent wrong answer or
downstream authorization appears. Phase 5 lives on the same capability
boundary and is unchanged by this phase.

## 12. Reliability contracts (Phase 7)

Phase 7 turns the reliability claims the architecture had written down into
contracts the code actually enforces, so persistent and eventually unattended
use is safe for the system's own state and resources.

### Memory budget and accounting (`src/reliability.rs`)

`MemoryBudget` declares the capacities the theorems are stated against
(clusters, entries per cluster, transient clusters, entries per transient
cluster, total bytes).  The growth paths consult it and **evict** instead of
appending past the cap:

* `VSABrain::add_to_dejavu_db` drains a quarter of a cluster's entries at the
  per-cluster cap, and at the cluster cap evicts the coldest cluster (remapping
  cold-storage blobs and cross-cluster associations so indices stay valid).
* `VSABrain::add_transient_fact` enforces both the per-transient entry cap and
  the transient-cluster count cap, evicting frozen clusters first.
* `MemoryCluster::novelty_gate_with_budget` is the budget-aware form of the
  novelty gate; the old `novelty_gate` delegates with the historical constant.

`VSABrain::memory_report` returns a `MemoryReport` that accounts for the whole
resident footprint: entry payloads, label/metadata strings, dense accumulators,
centroids and anchors, cross-cluster associations, experiences, live indexer
entries, and — via `account_conversation` — conversation sessions, turns,
pending clarifications, and per-session context.  This replaces the earlier
`MemorySnapshot`, which counted only cluster/entry counts and one accumulator
approximation and hard-coded several fields to zero.

The dedicated test `sustained_cluster_ingestion_respects_the_cluster_cap` and
its transient counterpart drive thousands of insertions under a tight budget
and assert the caps hold, rather than asserting a fixed fixture size.

### Replay taxonomy

`ReplayClass` separates the three strengths a "replay" check can have:

| Class | What it proves | Examples |
|---|---|---|
| `HashConsistency` | the recorded bytes did not change | `semantic_ir`, `*::replay_verified` receipt hashes |
| `Recomputation` | the same computation, re-run, agrees | `replay_linear_equation`, `replay_substitution` |
| `IndependentVerification` | a *different* mechanism checks the answer | `solution_verification`, algebra residual substitution |

`replay_catalogue()` is the curated registry; `cargo run --bin
reliability_report` emits the per-class counts so a consumer cannot present a
hash check as independent verification.

### Mathematical assumptions register (MATH.md §0.4)

The empirical-validation table gained an `Enforcement` column classifying each
assumption as **assumed**, **checked**, or **enforced**, with the witness naming
the enforcement point.  Memory-boundedness theorems (III.1, II.2) are now
runtime contracts, not merely asymptotic statements.

### Server hardening (`src/chat_server.rs`, `src/bin/machine_chat.rs`)

* **Localhost by default.** The bind host defaults to `127.0.0.1`.  A
  non-loopback `--host` is refused unless `--allow-remote` is given *and* a
  bearer token is configured.
* **Authentication.** `--token <value>` or `MACHINE_CHAT_TOKEN` sets a bearer
  token; the auth middleware then requires it on every `/api/*` request using a
  constant-time comparison.  Static assets stay open so the interface can load.
* **Bounded request sizes.** `DefaultBodyLimit` enforces `max_body_bytes`; turn
  text over `max_text_chars` is rejected before any work.
* **Execution timeout.** A turn has a wall-clock budget; exceeding it requests
  cooperative cancellation and reports an explicit `turn timed out` event.
* **Bounded queue / admission control.** A semaphore caps concurrent in-flight
  turns; a request that cannot be admitted within `queue_wait_timeout` receives
  `503 Service Unavailable` instead of piling up.
* **Panic-safe cleanup.** A `TurnGuard` removes the active-turn entry even if
  the blocking worker panics, so `active_turns` cannot leak.
* **Configuration honesty.** `ChatServerConfig` carries the token; the
  invariants of the supported configuration are reported exactly.

### Verification

```bash
cargo test --lib reliability::
cargo test --lib chat_server::
cargo run --bin reliability_report    # writes docs/phase7_reliability_v1.*
```

Measured 2026-09-28 by the report binary: 20,000 sustained observations, 24
clusters, all caps respected, accounted total 2.65 MiB against a 64 MiB
budget; replay classes 4 / 9 / 5 (hash / recompute / independent); 15
guarantees enforced, 2 checked, 0 assumed (the registry grows as later phases
add enforced guarantees, e.g. `G-TASK-AUTHORITY` and `G-TASK-OBSERVABLE` in
Phase 10 and `G-RELEASE-SCHEMA` in Phase 11); projection path
`cpu_soft_projection`.  Failures are reported when an enforced guarantee is
violated, so the report cannot claim more than the code does.

## 13. Document learning (Phase 8)

Phase 8 adds an inspectable path from a document to knowledge: import the text,
extract facts/definitions/rules with source locations, review what was
understood *and rejected*, commit the accepted items with provenance, answer
questions citing the source, and remove the document to invalidate exactly the
knowledge derived from it.  Scanned documents and visual interpretation are
explicitly out of scope for now.

### Extraction (`src/document_learning.rs`)

`extract_text` / `extract_pages` split the source into sentences that carry a
byte `SourceSpan` (and a page number when page boundaries were available).
Each sentence is offered to, in order, the rule reader (`if … then …`), the
definition reader (`X is a/the Y`, `X is called Y`, `X refers to Y`), and the
general SVO reader (`nlp::extract_svo`).  Whatever is understood becomes a
`ProposedItem` with a typed payload, a confidence, and its span; whatever is not
becomes a rejection with a reason (`no complete fact, definition, or rule`,
`duplicate of an earlier item`, `boilerplate or page furniture`, or
`instruction-like text treated as data, not executed`).  Nothing is silently
dropped.

### Imported text is data, never commands

A document is untrusted source material.  `classify_instruction` detects
second-person imperatives and well-known prompt-injection markers
("ignore previous instructions", "system:", "you must", "delete …", "curl …",
…).  Such sentences are recorded as rejected items with
`REASON_INSTRUCTION` and are never executed and never committed.  This is
defence in depth: the real boundary is that nothing is committed until a caller
explicitly accepts it.

### Propose, then commit on explicit acceptance

`ConversationService::import_text_document` / `import_document_path` /
`import_pdf_document` persist the document and its proposal but commit nothing.
`inspect_document` shows every item with its span, page, review status, and
reason.  `accept_document_item` / `reject_document_item` record a review;
`commit_document` turns the accepted items into durable assertions whose
provenance names the document (`source.kind = "document"`).  `learn_document` is
the one-step accept-then-commit convenience.  An instruction-like item can never
be accepted.

### Removal

`remove_document` retracts exactly the assertions whose items point back at the
document (reusing the Phase 3 retraction machinery, which also marks dependent
turns stale), clears them from the reasoning engine, and marks the document
`removed`.  Knowledge taught another way is untouched.  Re-importing the same
content is reported as a duplicate via the content hash.

### Storage (`src/persistence/documents.rs`, migration `0002_documents.sql`)

The schema version is now **2**.  `documents` records the document identity
(id, title, kind, origin, SHA-256, byte length, status); `document_items`
records each proposed/committed item with its span, page, review status,
reason, and the durable assertion id it produced.  Items cascade with the
document; removing a document never deletes the audit trail of what it
contained.

### Surfaces

* Library: `document_learning` module plus the `ConversationService` methods
  above.
* CLI: `cargo run --bin machine_docs -- demo` (or `import`, `list`, `inspect`,
  `accept`, `reject`, `commit`, `learn`, `ask`, `remove`).
* HTTP: `GET/POST /api/documents`, `GET /api/documents/{id}`,
  `POST /api/documents/{id}/learn`, `POST /api/documents/{id}/remove`,
  `POST /api/documents/items/{item_id}/accept|reject`.

### Verification

```bash
cargo test --lib document_learning::
cargo test --lib document                # module + service workflow tests
cargo test --lib persistence::tests::document
cargo test -p conversation_runtime_integration --locked   # Phase 8 acceptance
cargo run --bin machine_docs -- demo
```

The frozen acceptance test imports a small document, inspects the extracted
knowledge, asks a supported question (which abstains before commit and cites the
source after), and removes the document to confirm the influence disappears
predictably — including across a database reload.

## 14. Conversation evaluation (Phase 9)

Phase 9 makes the whole application part of the evaluation system: reproducible
conversation traces become versioned regression cases, an untouched evaluation
set is kept separate, and every mechanism is measured as a paired ablation. The
goal is to measure the complete application, **including its failures**.

### Corpus format (`data/conversation_eval_v1.json`, `data/conversation_eval_holdout_v1.json`)

Each corpus is a list of cases; each case is a list of steps with a session and
optional `restart_after`, plus a `gold` block:

```json
{ "input": "Who manages the observatory?",
  "gold": { "answered": true, "answer_contains": ["Alice"],
            "context_dependent": true, "stale_at_least": 2 } }
```

Gold may declare `answered`, `answer_contains` / `answer_not_contains`,
`clarification`, `context_dependent`, `stale_at_least`,
`evidence_provenance_contains`, and `capability`. The split tag
(`regression` / `holdout`) is part of the file.

### Trace capture and replay (`src/conversation_eval.rs`)

A run of the corpus is snapshotted to a trace: per turn, the outcome, answer
text, request kind, capability, evidence count, verification, follow-up
kind/source, latency, and knowledge counts. `detect_drift` compares two traces
field by field; volatile values (fresh provenance ids, latency) are normalized
so only behavioural change counts. `Trace::digest` is the stable SHA-256 of the
normalized trace.

### Metrics

`compute_metrics` reports answer correctness, coverage, unsupported assertions,
clarification success, context accuracy, correction propagation, latency
(mean/p50/p95/max), and memory. Two rules keep the numbers honest:

* **Oracle scoring is separate from system self-rejection.** A delivered answer
  judged right/wrong against gold is an oracle result; an abstention or
  clarification is the system's own decision, counted on its own. They are never
  summed, so a system rejection is never silently counted as a correct answer.
* **Unsupported assertions** are delivered *answers to questions* with no
  evidence. Teaching acknowledgements and clarifications are not assertions.
  The declared limit is 0 and run-time breach fails the gate.

### Ablations

`ConversationService::EvalConfig` carries six toggles that default to production
behaviour: `use_vsa_retrieval` (falls back to lexical exact-term retrieval),
`use_context_memory`, `use_typed_capabilities`, `use_reuse`, `use_semantic_worker`
(the shadow worker offers a stored reading as a clarification instead of
abstaining), and `use_consolidation` (feeds the turn back into the bounded
working context). `run_ablations` runs the same corpus with each mechanism
enabled and disabled and reports both metric sets plus the signed delta, so a
baseline is never hard-coded.

### Gate

`cargo run --bin machine_eval` writes
`docs/phase9_conversation_eval_v1.report.json` and `.md` (plus the frozen
regression and holdout traces) and exits non-zero on regression drift, an
unsupported assertion beyond the limit, or an ablation that fails to preserve
safety. The holdout set is reported but not gated.

### Verification

```bash
cargo test --lib conversation_eval
cargo test -p conversation_runtime_integration --locked   # includes Phase 9
cargo run --bin machine_eval            # score + gate
cargo run --bin machine_eval replay     # drift check against the frozen trace
```

## 15. Controlled autonomy (Phase 10)

Phase 10 lets the conversation initiate useful background work now that
answering and memory are dependable. The work is deliberately bounded: every
task declares a scope and budget up front, records its progress, can stop and
resume, and explains its result. It reuses the existing action and planning
infrastructure through explicit adapters and keeps chat independent of
unrestricted shell execution and the full simulation loop.

### Task model (`src/autonomy_task.rs`)

A [`Task`] is planned from a `TaskKind`, a `TaskInput`, and a `TaskBudget`:

| Kind | Input | Adapter (reused infrastructure) |
|---|---|---|
| `AnalyzeDocument` | title + text | `document_learning::extract_text` (read-only; never commits) |
| `Investigate` | a `ResearchCase` | `mathematical_research::run_case` (replay-verified) |
| `RunExperiment` | candidate count + budget | `capability_planner` portfolio selection (planning only) |
| `ConsolidateMemory` | none | `reliability::account_conversation` over the live service |
| `ProduceReport` | a task id | summarizes that task's own progress and result |

The `TaskHost` trait is the seam. `DefaultTaskHost` reaches only the library's
read-only/planning modules; `ConversationTaskHost` reads the conversation's own
sessions and turns so a consolidation task observes the real chat state. Neither
can reach a shell, a network, or the simulation loop, because no `Capability`
enum variant corresponds to those.

### Authority and budget

`AuthorityScope` is an allowlist of `Capability` values; `TaskBudget` pairs it
with a `cognition::AutonomyBudget` (actions, wall-clock, external writes, max
risk) and a step cap. `AuthorityScope::for_kind` grants exactly the capabilities
a kind needs. The scope is checked twice: once at planning (does the budget
cover the required capabilities?) and once per step (is the capability about to
run inside the scope?). A breach is a `TaskState::Refused` — the capability is
never invoked, so "within authority" stays true even when a task is stopped for
wanting more.

### Lifecycle

`TaskState` is `Pending → Running → {Paused → Running}* → Completed | Cancelled |
Refused`. The runner is deterministic and stepwise: `start`, then repeated
`step` (one capability use each) or `run`. `pause`/`resume` continue from the
next step; `cancel` is cooperative. Every step appends a `TaskProgress` record,
so a stopped task carries its history. The completion condition is exact: the
task kind's ordered step plan is exhausted and a result exists.

### Report

`TaskReport` (and `TaskSuiteReport` for the evaluated suite) explains the run:
state, steps, budget spent, whether it stayed within budget and authority, the
progress log, and the result. `machine_eval task` runs the canonical suite and
writes `docs/phase10_autonomy_v1.report.json` / `.md`, exiting non-zero if any
task exceeded its declaration or failed to explain its result.

### Verification

```bash
cargo test --lib autonomy_task
cargo test -p conversation_runtime_integration --locked   # includes Phase 10
cargo run --bin machine_eval task                          # suite + gate
```

## 16. Dependable release (Phase 11)

Phase 11 makes everyday operation straightforward: one documented startup
command, a clear first-run setup, configuration validation, health and
dependency status, backup/restore/upgrade procedures, a versioned capability
inventory, release notes tied to evaluation results, optional GPU and
semantic-worker profiles, and a recovery path after a failed upgrade. Everything
operational lives in `src/operator.rs`; `src/bin/machine.rs` is a thin argument
parser. The repository stays a single crate because build-time and dependency
boundaries do not yet justify a split, and a broad restructuring must not delay
the first usable chat.

### One command, first-run setup

```bash
cargo run --release --bin machine -- setup   # once: create dirs, open the DB, doctor
cargo run --release --bin machine -- start   # daily use: the chat interface
```

`setup` creates the data and backup directories, opens the database once (which
runs migrations), and prints the health summary and the exact next command.
`start` validates the configuration, prints warnings from the doctor, binds, and
serves the Phase 2 interface. `machine_chat` is retained for research-only flag
parity; the operator path is the documented one.

### Configuration and validation

`OperatorConfig` is built from `MACHINE_*` environment variables and flags
(`--data-dir`, `--db`, `--memory`, `--store`, `--backup-dir`, `--host`, `--port`,
`--allow-remote`, `--token`, `--profile`, `--semantic-worker`,
`--memory-budget`). `OperatorConfig::validate` returns `ConfigIssue`s, each with
a severity, the field, and a concrete remedy; `machine config check` prints them
and exits non-zero on any error. Validation is side-effect free and covers: a
zero port, empty data/database paths, a non-loopback bind without `--allow-remote`
and a token, the `gpu` profile on a binary built without CUDA, the
`semantic_worker` profile with the worker disabled, a very small memory budget,
and a database placed outside the data directory.

### Health and dependency status (`machine doctor`)

`doctor(config)` returns a `DoctorReport` of ordered checks: the release and
schema, the profile, the active projection path, the bundled SQLite and
pdf-extract backends, data-directory writability, database integrity and schema
compatibility (read-only, no migration), available legacy imports, CUDA/profile
match, semantic-worker state, port availability, and every configuration issue.
Each check carries a status (`ok` / `warn` / `fail`), a detail, and an optional
remedy. `machine doctor` is healthy only when no check fails.

### Versioned capability inventory

`CAPABILITIES` is a versioned inventory (`phase11-capability-inventory-v1`,
release `v3.4`). Each entry records the id, title, status
(`stable`/`active`/`experimental`/`shadow`/`deferred`), the phase that introduced
it, the `docs/CLAIMS.md` claim id when one exists, the committed evaluation
artifacts that back it, the command that exercises it, and its surfaces.
`machine capabilities` prints it; `docs/phase11_capability_inventory_v1.json` is
the generated artifact.

### Release notes tied to evaluation results

`release_notes()` reads the committed evaluation reports and reports what was
actually measured — the reliability report's guarantee counts and projection
path, the conversation-evaluation regression/holdout coverage and ablation
count, the semantic-fidelity wrong-answer limit, and the autonomy suite's
scenario counts — rather than hard-coded text. `machine release notes` renders
the Markdown; `machine release write` regenerates
`docs/phase11_capability_inventory_v1.json`, `docs/phase11_release_notes_v1.md`,
and `docs/phase11_doctor_v1.json`.

### Optional GPU and semantic-worker profiles

`Profile` is a documented configuration, not a separate build. `standard` is the
default; `minimal` omits the web interface; `gpu` expects a `cargo build
--features cuda` binary; `semantic_worker` enables the shadow worker. `doctor`
and `config check` report honestly when the running binary does not match the
requested profile instead of pretending the backend is present.

### Backup, restore, upgrade, and recovery

```bash
machine backup  /path/backup.db
machine restore /path/backup.db
machine upgrade            # or `machine upgrade --dry-run` to plan only
machine recover
```

`machine upgrade` refuses a database newer than the build *before touching it*,
takes a timestamped pre-upgrade backup in `--backup-dir`, then opens the
database (running migrations) and verifies integrity and the resulting schema.
`machine recover` restores the most recent backup and verifies it. Backups use
SQLite's online backup API without opening or migrating the source, so they are
safe on a live database. This is the enforced guarantee `G-RELEASE-SCHEMA`:
"a database is never opened for migration without a pre-upgrade backup, and a
database newer than the running build is refused rather than misread."

### Verification

```bash
cargo test --lib operator
cargo test -p conversation_runtime_integration --locked   # includes Phase 11
cargo run --bin machine -- doctor
cargo run --bin machine -- release write                   # regenerate artifacts
```

The frozen acceptance test `release_packaging_validates_health_inventory_and_upgrade_recovery`
validates the default configuration and rejects a non-loopback bind without
opt-in, confirms the inventory is versioned and the notes are tied to committed
artifacts, sets up and doctors a fresh database, upgrades with a backup, recovers
from it, and confirms an upgrade refuses a forged newer database without
modifying it.

## 17. Baseline benchmark

Small, deterministic proposition/proof-kernel benchmark:

```bash
cargo build --release --locked -j 3 --bin proposition_bench   # 16m17s from clean
cargo run --release --locked --bin proposition_bench -- \
    100 42 results/phase0_baseline/proposition_100_42_run1.jsonl local
```

Recorded 2026-09-18 (Intel i7-1365U, portable release build):

| Metric | total | development | holdout |
|---|---|---|---|
| cases | 100 | 80 | 20 |
| expected accepts / accepted / replay | 65 / 65 / 65 | 52 / 52 / 52 | 13 / 13 / 13 |
| false accepts / false rejections | 0 / 0 | 0 / 0 | 0 / 0 |
| acceptance rate | 0.65 | 0.65 | 0.65 |
| replay rate | 1.0 | 1.0 | 1.0 |
| wall time | ~10 ms | | |

`run1` and `run2` output files are semantically identical (verified by
parsing the JSONL). Byte-level hashes differ only because `serde_json`
serializes the metrics `HashMap` in a non-deterministic key order.

## 18. CUDA (optional, unavailable here)

`build.rs` no longer hardcodes the previous machine's GPU architecture or pip
paths. When `--features cuda` is enabled it:

1. discovers the toolkit from `$CUDA_PATH`, `$CUDA_HOME`, `/usr/local/cuda*`,
   `/opt/cuda` (Arch), or pip `nvidia-*` packages under
   `~/.local/lib/python*/site-packages/nvidia/*`;
2. derives the target from `$CUDA_ARCH` (accepts `120`, `12.0`, `sm_120`) or
   `nvidia-smi --query-gpu=compute_cap`;
3. locates `libcudart.so` in `lib`, `lib64`, or `targets/x86_64-linux/lib`.

If the toolkit or architecture is missing it emits explicit warnings and skips
the kernel build, leaving the CPU build untouched.

## 19. Quick reference

```bash
# portable library build + tests
cargo build --locked --lib -j 3
cargo test  --lib --locked -j 3

# locally optimized (non-portable)
cargo build --release --locked --lib --config .cargo/config.native.toml

# Phase 1 conversational runtime
cargo test --lib conversation
cargo test -p conversation_runtime_integration --locked

# Phase 2 chat interface
cargo run --bin machine_chat -- --open
cargo test --lib chat_server

# Phase 3 durable state
cargo test --lib persistence
cargo run --bin machine_chat -- --backup /tmp/machine-backup.db
cargo run --bin machine_chat -- --restore /tmp/machine-backup.db

# Phase 4 follow-up conversation
cargo test --lib conversation::followup
cargo test -p conversation_runtime_integration   # includes frozen phase4 scenarios

# Phase 5 connected capabilities
cargo test --lib conversation::capability_adapter
cargo test -p conversation_runtime_integration   # also runs frozen phase5 scenarios

# Phase 6 semantic fidelity + shadow worker connection
cargo test --lib semantic_fidelity
cargo test --lib semantic_shadow
cargo run --bin semantic_fidelity_eval           # writes docs/semantic_fidelity_eval_v1.*

# Phase 7 reliability contracts
cargo test --lib reliability
cargo test --lib chat_server
cargo run --bin reliability_report               # writes docs/phase7_reliability_v1.*
cargo run --bin machine_chat -- --token <value>  # bearer token on /api/*
cargo run --bin machine_chat -- --host 0.0.0.0 --allow-remote --token <value>

# Phase 8 document learning
cargo test --lib document_learning
cargo test --lib document
cargo run --bin machine_docs -- demo
cargo run --bin machine_docs -- import <path>       # then list / inspect / learn / ask / remove
cargo test -p conversation_runtime_integration      # includes Phase 8 acceptance

# Phase 9 conversation evaluation
cargo test --lib conversation_eval
cargo run --bin machine_eval                       # score + gate (writes docs/phase9_*)
cargo run --bin machine_eval capture               # (re)write frozen traces
cargo run --bin machine_eval replay                # drift check against frozen traces
cargo test -p conversation_runtime_integration     # includes Phase 9 acceptance

# Phase 10 controlled autonomy
cargo test --lib autonomy_task
cargo run --bin machine_eval task                  # suite + gate (writes docs/phase10_autonomy_v1.*)
cargo test -p conversation_runtime_integration     # includes Phase 10 acceptance

# Phase 11 dependable release
cargo run --bin machine -- setup                   # first run: dirs + DB + doctor
cargo run --bin machine -- start                   # daily use (single command)
cargo run --bin machine -- doctor                  # health + dependency status
cargo run --bin machine -- config check            # validate configuration
cargo run --bin machine -- capabilities            # versioned inventory
cargo run --bin machine -- release notes           # release notes tied to eval results
cargo run --bin machine -- release write           # regenerate docs/phase11_*
cargo run --bin machine -- upgrade                 # back up, then migrate
cargo run --bin machine -- recover                 # restore the latest backup
cargo test --lib operator
cargo test -p conversation_runtime_integration     # includes Phase 11 acceptance

# baseline benchmark
cargo run --release --locked --bin proposition_bench -- \
    100 42 results/phase0_baseline/proposition_100_42.jsonl local
```
