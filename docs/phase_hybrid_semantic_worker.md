# Hybrid semantic worker boundary

The semantic IR is now paired with a model-runtime-neutral worker client. It
can call a local OpenAI-compatible `llama.cpp` endpoint and records an
immutable raw receipt containing the input, model, endpoint, configuration,
prompt, grammar, and raw-output hashes.

The worker does not solve a problem, select a capability, or authorize a
result. JSON decoding is separate from deterministic semantic validation. A
candidate must carry the versioned IR schema and is stamped with trusted worker
metadata before `semantic_ir::validate_candidate` examines it.

Scheduling is explicit and lexical/domain blind:

* short, low-ambiguity jobs use the fast 5070 tier;
* long, high-ambiguity, or multi-candidate jobs escalate to the P40 tier.

The worker configuration can carry llama.cpp grammar text. When present, it is
sent as a constrained-decoding request and included in the configuration hash;
when absent, the endpoint remains compatible with a server-side grammar policy.

The boundary distinguishes semantic replay (replaying a stored proposal and
validator decision) from generation replay (asking a model to regenerate the
same bytes). Only semantic replay is required for authorization.

Focused tests cover tier routing, prompt safety, configuration/receipt hashing,
candidate decoding, and tamper detection. An OpenAI-compatible HTTP smoke test
is included for integration environments but is ignored in this sandbox because
local socket binding is prohibited. No registry, solver, or production route is
changed.
