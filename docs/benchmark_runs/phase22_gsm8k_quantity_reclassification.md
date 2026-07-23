# Phase 22 — GSM8K QuantityRelation candidate reclassification

This release re-evaluates the source-preserved 100-case GSM8K restricted
release from Phase 19 after the QuantityRelation vertical was integrated
through typed algebra bridges.  The original v2 release is not modified.  The
candidate manifest is locked to base release hash
`c3d288f3ecf5f4bf1bbc6e8160e0edb1cf469e2df0db21f3476caddd330b1374` and has
configuration SHA-256
`4322ed3a6cf6a803dc1da2cb2ab229f0913a10d53fb833b9232968bf93065303`.

The candidate adapter is diagnostic-only.  It recognizes eight
source-preserved, explicitly reviewed quantity/rate/conversion stories and
turns them into `QuantityRelationArtifact` objects.  It does not change the
global router, authorization defaults, or the original GSM8K release labels.
Each accepted candidate is handed to the existing algebra executor and must
replay successfully.

## Candidate release

```text
RUSTFLAGS='-Awarnings' cargo run --release --quiet --bin gsm8k_quantity_candidate_bench -- data/third_party_gsm8k_quantity_candidate_v1.json
```

```text
cases=100
structural=100/100
existing=4
quantity_expected=8
quantity_realized=8
quantity_replayed=8
ambiguous=2
unsupported=86
results=12/12
false_auth=0
false_denials=0
candidate_leakage=0
failures={}
```

The supported slice therefore grows from four existing routes to twelve
accepted routes, while the two ambiguous cases remain ambiguous and the 86
remaining unsupported cases remain rejected.  The eight new routes cover
explicit unit rates, unit conversions, and linear quantity arithmetic.  They
are not a claim of general GSM8K solving.

## Safety and scope

- QuantityRelation artifacts are replay-verified before the algebra bridge.
- Algebra receipts are independently replay-verified.
- No unsupported or ambiguous source case matched the candidate adapter
  (`candidate_leakage=0`).
- The original v2 release remains immutable and continues to report 4/4
  supported cases, 2 ambiguities, 94 unsupported cases, and zero authorization
  errors.
- The candidate adapter is intentionally separate from production routing;
  a later release must evaluate a broader, independently reviewed candidate
  corpus before changing global routing.

This closes the first measured capability-growth loop on an external slice:

```text
external failure cluster
→ QuantityRelation contract
→ bounded implementation
→ typed bridge
→ source-preserved reclassification
→ exact replay evidence
```
