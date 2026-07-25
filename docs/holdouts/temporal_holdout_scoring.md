# Temporal Holdout Scoring Report

**Execution**: 2026-07-25, single execution  
**Proposer frozen at**: `556a6e50cc1dbdc447f82868dc5b956a830219ab` (Phase 2G)  
**Rubric pre-registered at**: `e9521a2`  
**Evidence hash**: `0ed0b083aefdad60242878de18fafd83703df188db2201729e57dd1c0477a5ab`  
**Raw output**: `docs/holdouts/temporal_holdout_raw_output.txt`

---

## Summary

**24 temporal_or_sequential_reasoning prompts** from GSM8K restricted v2 were
passed to `propose_from_failures(threshold=0.30)`. The proposer returned
**2 proposals** in 5.2ms.

| Proposal | Cluster size | Invariant | Applicable | Ambiguous | Unsupported |
|----------|-------------|-----------|-----------|-----------|-------------|
| #1 (AdditiveChange) | 12 | "Arithmetic relation among explicit quantities" | 1 | 0 | 23 |
| #2 (PartOfWhole) | 11 | "Single-step linear percentage transformation on an explicit base quantity" | 6 | 0 | 18 |

The 24th case (gsm8k.v2.0023, "candle melts by 2 cm/hour 1PM→5PM") was assigned
to Proposal #1 but classified Unsupported. It is the only case already marked
as Supported in the corpus.

---

## Dimension Scores

### D1 — Abstraction Quality: **Absent**

Neither proposal identifies a coherent temporal transformation:

- **Proposal #1** claims the invariant is "Arithmetic relation among explicit
  quantities" — which is generic math. It captures cases involving rates
  (eggs/day, miles/hour, cm/hour) but strips away the temporal structure.
  The supported form is `additivechange` with `unit_bearing_scalar` required
  feature — this describes units (dollars, meters, liters) but says nothing
  about time intervals, elapsed duration, or sequential state.

- **Proposal #2** claims "Single-step linear percentage transformation on an
  explicit base quantity" — this is the existing percentage capability pattern.
  Some temporal cases (month-over-month percentages, sequential discounts)
  were absorbed here because they contain percentage keywords.

The proposer does not recognise that these cases share an underlying temporal
transformation. It maps them to existing non-temporal arithmetic patterns.

**Required for Adequate**: Recognise that the cases involve sequential or
time-based arithmetic. Not met.

---

### D2 — Typed Contract: **Weak**

Both proposals use:
- Inputs: `[NumericQuantity]` or `[NumericQuantity, PercentageRate]`
- Outputs: `[QuantityRelation]`

These types could describe any arithmetic operation — subtraction, division,
unit conversion. They lack any temporal dimension:

- No `Duration`, `TimeInterval`, or `StepCount` input type
- No `ElapsedTime`, `AccumulatedQuantity`, or `SequencePosition` output type
- No distinction between rate accumulation and one-shot arithmetic

**Required for Adequate**: ≥2 input types and ≥1 output type semantically
relevant. Partially met by Proposal #2 having 2 input types, but they are
not temporally relevant.

---

### D3 — Supported Boundary: **Weak**

Proposal #1 supports only **1 of 24** cases as Applicable (4.2% supported
precision by the intended capability). The single applicable case (Marissa
hiking) is correctly classified as AdditiveChange, but this means 22/23
temporal cases are rejected that should be accepted.

Proposal #2 supports **6 of 24** cases (25%), but these are the percentage/
finance sub-group (cases with "percent", "discount", "initial amount").
These are not genuinely temporal — they were absorbed because of percentage
keywords.

Supported precision from a temporal perspective: **~4%** across both proposals.
No supported forms distinguish temporal from non-temporal cases.

**Required for Adequate**: ≥60% supported precision. Not met.

---

### D4 — Ambiguity Handling: **Absent**

**0 ambiguous cases** across both proposals (0/24 decisions are Ambiguous).
Expected ambiguity types that should have been recognised:

- Missing start time or reference point (e.g., "over" without clear interval)
- Unresolved AM/PM or day boundary  
- Ambiguous "remaining" (after which step?)
- Inclusive vs exclusive counting ("after the third step")
- Unspecified initial state

None were detected. Ambiguity causes are systematically collapsed into
Unsupported.

**Required for Weak**: At least default "missing binding" ambiguity. Not met.

---

### D5 — Unsupported Neighbors: **Adequate**

The proposals correctly exclude several unrelated domains:

| Excluded Domain | Predicate | Correct? |
|-----------------|-----------|----------|
| Probability/likelihood | `ForbidsLikelihoodSemantics` | ✅ |
| Compound growth (each year) | `ForbidsRepeatedTemporalApplication` | ✅ |
| Financial constructs | `ForbidsFinancialConstructs` | ✅ |
| Incompatible units | `ForbidsIncompatibleUnits` | ✅ |
| Abstract symbolic | `ForbidsAbstractSymbolicExpression` | ✅ |

However, these exclusions cast too wide a net: they reject valid temporal
cases alongside genuinely unrelated ones. For example, "per day" rate
problems are rejected by `ForbidsIncompatibleUnits` because they lack
compatible unit conversion semantics.

**Required for Adequate**: Excludes 2–3 families with ≥60% unsupported
precision. Met for the excluded domains, but the precision analysis is
complicated by the fact that legitimate temporal cases are also rejected.

---

### D6 — Existing-Capability Reuse: **Adequate**

The proposer correctly split the temporal evidence into existing capability
patterns. It did NOT claim "temporal reasoning" as a novel single capability,
which would have been a false positive. Instead, it mapped cases to:

- **AdditiveChange**: cases with rate arithmetic and unit-bearing scalars
- **PartOfWhole/Percentage**: cases with sequential percentage operations

This is structurally correct: the temporal cases ARE cross-contaminated with
rate arithmetic and percentage patterns. However, the proposer did NOT
recognise that these cases ALSO have a shared temporal dimension that neither
existing capability fully captures.

The novelty check correctly returned `true` (is_novel) for both proposals,
but the reasoning ("No existing capability matches this feature profile")
overlooks the overlap with existing capabilities.

**Required for Adequate**: Notes overlap with 1 existing capability. The
proposer implicitly does this by clustering into existing patterns rather
than creating a new one. However, it does not explicitly name the existing
capability or propose a bridge.

---

### D7 — Validation Plan Quality: **Absent**

No validation plan was generated. Neither proposal specifies:

- Positives across each supported form
- Ambiguity cases  
- Near-miss categories
- Rewrite families
- Adversarial cross-domain cases

The proposal format does not currently include a validation plan field.

---

## Outcome Category: **ClusterShouldSplit**

**Determination**: The evidence does NOT support a single `temporal_or_sequential_reasoning`
capability. The proposer correctly split the 24 cases into existing patterns
(AdditiveChange + PartOfWhole/Percentage), confirming that the taxonomy label
`temporal_or_sequential_reasoning` was a topic-level description rather than
a grounded transformation type.

The GSM8K labelling heuristic (`rejection_cluster` in `third_party_corpus_benchmark.rs`)
is keyword-based: it tags cases containing "every day", "per day", "over ",
"after ", or "remaining" as temporal. These keywords trigger the temporal
label even when the underlying math is:
- Simple rate arithmetic (eggs/day × days = total)
- Sequential subtraction (remaining after removing portions)
- Month-over-month percentage change (when "month" is mentioned)

These ARE distinct transformations that existing capabilities already handle.
The proposer correctly refused to invent a "temporal" capability that would
have been an unjustifiably broad contract.

### Limitations Acknowledged

1. **No temporal abstraction discovered**: The proposer did not identify
   elapsed-time calculation, ordered event sequence, or state transition
   as the shared transformation — it fell back to generic arithmetic. This
   is a genuine gap in the feature extractor's ability to detect temporal
   relations.

2. **No ambiguity detected**: The systematic collapse of ambiguous cases to
   Unsupported is a known Phase 2G limitation (also present in historical
   reconstructions for some tasks).

3. **Supported recall is extremely low**: Only 1/24 cases accepted by the
   temporal-adjacent proposal. This is because the form-matching requires
   exact feature matches that the temporal prompts don't satisfy.

4. **Exclusion recall was weak in the campaign too**: The Phase 2G campaign
   showed exclusion recall at 26.7–33.3% for most tasks. The holdout's
   exclusion coverage is consistent with this.

### Scientific Value

This is an informative negative result. The proposer demonstrates that:

> The GSM8K `temporal_or_sequential_reasoning` residual unsupported cluster
> is not a coherent capability family. It is a topic-level aggregation of
> cases whose underlying transformations (rate arithmetic, sequential
> subtraction, percentage change) are already addressable by existing
> quantity-relation patterns.

The proposer's refusal to create a single temporal capability is arguably
**correcter than the human-supplied taxonomy label**, which grouped cases
by vocabulary ("every day", "per day") rather than by shared transformation.

### What Would Be Needed for a Genuine Temporal Capability

The evidence suggests that a true temporal transformation (elapsed-time
calculation, state transitions over steps, ordered event sequences) might
require:

1. **Temporal feature extraction in SemanticFeatures**: The current extractor
   has `RepeatedChange` semantics for "each year" / "over " patterns, but
   no `TimeInterval` or `ElapsedDuration` semantics. Adding a dedicated
   `temporal_relation` field with variants like `ElapsedTime`, `SequentialState`,
   `IntervalRate` would let the proposer distinguish temporal from
   non-temporal arithmetic.

2. **Dedicated temporal evidence**: The 24 GSM8K cases are mostly rate
   arithmetic that happens to mention days/weeks/years. A set of genuinely
   temporal prompts (e.g., "What time will it be 3 hours after 2:30 PM?",
   "Sort the events: breakfast at 8, meeting at 10, lunch at 12") would
   provide evidence for a distinct temporal transformation.

3. **Form improvements for sequential patterns**: The `AdditiveChange` form
   could be refined to admit a `multi_step` sub-form that captures
   sequential state transitions, with temporal interval as an optional
   dimension.

---

## Integrity Confirmation

- [x] Proposer logic frozen at commit 556a6e5 before holdout
- [x] Evaluation rubric committed at e9521a2 before holdout
- [x] Single execution: `cargo test temporal_holdout_single_execution --lib -- --nocapture`
- [x] Raw output saved to `docs/holdouts/temporal_holdout_raw_output.txt`
- [x] Evidence hash recorded: `0ed0b083aefdad60242878de18fafd83703df188db2201729e57dd1c0477a5ab`
- [x] Scoring completed against pre-registered rubric
- [x] No proposer modifications made after holdout execution
- [x] All subsequent changes are development for a new holdout
