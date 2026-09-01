# Semantic worker probe

`semantic_worker_probe` is the first executable integration point for a local
llama.cpp semantic parser. It accepts a technical problem, calls the configured
OpenAI-compatible worker, decodes its bounded candidate JSON, and applies the
deterministic ensemble validator.

Example configuration:

```text
SEMANTIC_WORKER_ENDPOINT=http://127.0.0.1:8081 \
SEMANTIC_WORKER_MODEL=semantic-parser \
SEMANTIC_WORKER_TIER=fast5070 \
cargo run --quiet --bin semantic_worker_probe -- "find the determinant of ..."
```

Use `SEMANTIC_WORKER_TIER=deepp40` for the fallback worker. The probe emits raw
and semantic receipts only; it never invokes a solver, authorizes an answer,
mutates a registry, or updates world state. The worker endpoint must provide a
JSON response at `/v1/chat/completions`.

Stored outputs can be evaluated later without a live model:

```text
SEMANTIC_EVAL_INPUT=worker_outputs.jsonl \
SEMANTIC_EVAL_OUTPUT=/tmp/semantic_eval.jsonl \
cargo run --quiet --bin semantic_worker_eval
```

Each input line contains `id`, `input`, and `raw_output`. The evaluator applies
the same decoder, ensemble gate, and replay checks, and reads no answer keys.
