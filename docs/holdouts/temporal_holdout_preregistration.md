# Temporal Holdout Pre-registration

**Date**: 2026-07-25  
**Frozen proposer commit**: `556a6e50cc1dbdc447f82868dc5b956a830219ab` (Phase 2G)
**Pre-registration commit**: *this file — hash assigned at commit time*  
**Holdout seed**: GSM8K restricted release v2, 24 temporal_or_sequential_reasoning cases  
**Proposer version**: Phase 2G (boundary synthesis with applicability decision model)  
**All proposer thresholds frozen**: Yes

---

## Pre-registered Evaluation Rubric

Score the proposer's output on these dimensions. Each dimension is scored
independently on a 4-level scale: **Strong**, **Adequate**, **Weak**, **Absent**.

### D1 — Abstraction Quality
*Does the proposal identify a coherent temporal transformation rather than
merely naming "time problems"?*

| Level | Criteria |
|-------|----------|
| **Strong** | Identifies one of: elapsed-time calculation, ordered event sequence, repeated interval progression, calendar/clock transformation, or state transition over steps. Does NOT lump unrelated temporal patterns together. |
| **Adequate** | Recognises that the cases involve sequential or time-based arithmetic but conflates 2–3 distinct temporal sub-patterns (e.g., elapsed time + repeated rates). |
| **Weak** | Produces a generic label like "time problems" or "temporal reasoning" with no internal structure. Alternatively splits the evidence into unrelated fragments. |
| **Absent** | No coherent temporal transformation is identified — the proposal is for an unrelated capability (finance, quantity, etc.) or the proposer returns no result. |

### D2 — Typed Contract (Inputs / Outputs)
*Does the proposal specify a sensible typed contract for the capability?*

Expected reasonable types for temporal/sequential reasoning:

```
Inputs:
  - ordered events or initial state (numeric or described)
  - explicit temporal relation (each day, per year, over interval)
  - duration, interval, or step count
  - requested temporal target

Outputs:
  - elapsed duration or accumulated quantity
  - event ordering or state at step
  - typed temporal relation
```

| Level | Criteria |
|-------|----------|
| **Strong** | Contract specifies ≥3 reasonable input types and ≥2 output types that match the evidence. Types distinguish temporal from non-temporal dimensions. |
| **Adequate** | Contract has ≥2 input types and ≥1 output type that are semantically relevant, but some types are vague (`NumericQuantity` without temporal qualifier). |
| **Weak** | Contract has only 1 input type and 1 output type, or uses generic types that could describe any arithmetic operation. |
| **Absent** | No typed contract is produced, or types mismatch the evidence entirely. |

### D3 — Supported Boundary (What Forms Are Identified)
*Does the proposal identify which temporal forms are genuinely realizable,
with explicit supported forms?*

| Level | Criteria |
|-------|----------|
| **Strong** | Identifies ≥2 distinct supported forms with explicit required features. Supported precision ≥80% on the holdout cases (where precision = fraction of cases labelled Applicable that genuinely belong). |
| **Adequate** | Identifies 1–2 supported forms. Supported precision ≥60%. Forms capture the main temporal pattern but miss variation. |
| **Weak** | One generic form that absorbs most or all cases — supported precision <60% or forms are too broad to reject any case. |
| **Absent** | No supported forms extracted, or all evidence classified as ambiguous/unsupported. |

### D4 — Ambiguity Handling
*Does the proposal recognize ambiguity cases correctly?*

Expected ambiguity types for temporal reasoning:
- Missing start time or reference point
- Unresolved AM/PM or day boundary
- Multiple possible event orderings
- Unclear inclusive vs exclusive counting ("after the third step")
- Missing initial state
- Ambiguous meaning of "over" (duration vs division)

| Level | Criteria |
|-------|----------|
| **Strong** | Identifies ≥3 distinct ambiguity causes. Ambiguity recall ≥50% on held-out judgments. |
| **Adequate** | Identifies 1–2 ambiguity causes. Ambiguity recall ≥25%. |
| **Weak** | Only default "missing binding" or "unresolved reference" identified. |
| **Absent** | No ambiguous cases recognized at all (systematically collapsed). |

### D5 — Unsupported Neighbors (What Is Correctly Rejected)
*Does the proposal correctly exclude unrelated domains?*

Must avoid absorbing:
- Recurrence execution merely because it mentions steps
- Rates merely because they involve time
- Date arithmetic requiring external calendar knowledge
- Probabilistic temporal events
- Compound finance over time (unless genuinely temporal)
- Causal inference from event order alone

| Level | Criteria |
|-------|----------|
| **Strong** | Explicitly excludes ≥4 of the above families. Unsupported precision ≥80%. |
| **Adequate** | Excludes 2–3 families. Unsupported precision ≥60%. |
| **Weak** | Excludes 1 family or only generic "not applicable" catch-all. |
| **Absent** | No explicit exclusions, or rejects valid temporal cases. |

### D6 — Existing-Capability Reuse
*Does the proposal recognize overlap with existing capabilities
(recurrence, quantity relations, algebra) and propose a bridge rather
than duplicating?*

| Level | Criteria |
|-------|----------|
| **Strong** | Identifies ≥2 existing capabilities that partially overlap. Proposes a bridge or extension (e.g., "extends QuantityRelation with temporal duration") rather than a new standalone capability. |
| **Adequate** | Notes overlap with 1 existing capability but proposes a new capability anyway. |
| **Weak** | No overlap detected even though overlap exists. Or proposes standalone when bridge would be appropriate. |
| **Absent** | Does not reference existing capabilities at all. |

### D7 — Validation Plan Quality
*Does the proposal generate a credible corpus specification for testing
the capability?*

| Level | Criteria |
|-------|----------|
| **Strong** | Specifies positives across each supported form, ambiguity cases, structural and lexical near-misses, ordering and indexing mutations, rewrite families, and adversarial cross-domain cases. |
| **Adequate** | Specifies at least positives and ambiguity cases. Missing some near-miss or adversarial categories. |
| **Weak** | Only positives specified, or corpus is just the input evidence recycled. |
| **Absent** | No validation plan. |

---

## Pre-registered Outcome Categories

The final outcome MUST be one of:

| Category | Definition |
|----------|------------|
| **UsefulNovelContract** | D1 Strong OR Adequate AND D2 Strong AND D3 Strong. The core transformation, typed boundary, and major safety distinctions are independently judged useful. |
| **UsefulButBoundaryIncomplete** | D1 Strong OR Adequate AND D2 Adequate AND D3 Adequate. The abstraction is correct, but important ambiguities or unsupported neighbors are missing. |
| **ExistingCapabilityExtension** | D6 Strong. The evidence supports extending recurrence, quantity, or another capability rather than creating a new one. |
| **ClusterShouldSplit** | D1 Weak because the proposer grouped several transformations that need distinct contracts. |
| **NoCoherentCapability** | D1 Absent AND D2 Weak OR Absent. The evidence does not support a single reusable abstraction. |

---

## Human Comparison Contract (Written Independently, Pre-holdout)

The following is written WITHOUT having seen the proposer's output. It is my
independent judgment of what a well-designed temporal/sequential reasoning
capability SHOULD look like, based solely on reading the 24 holdout prompts.

### Manual Analysis of the Holdout Evidence

The 24 GSM8K cases labelled `temporal_or_sequential_reasoning` fall into
several distinct patterns:

**Group A — Daily Rate × Elapsed Time (8 cases)**
- Case 0: 16 eggs/day, eat 3 + bake 4, sell remainder daily
- Case 1: Feed chickens 3 cups/day in 3 feedings
- Case 9: 1 serving/night, 15 servings/carton, 60 days
- Case 11: 2 yogurts/day, 4 for $5, 30 days
- Case 14: 252 eggs/day, $2/dozen, per week
- Case 21: 5 classes/day weekdays + 8 Saturday, per week
- Case 22: Feed puppy 1 cup/day for 180 days then 2 cups/day
- Case 4: 7 lemons/year, $1.5 each, $3/year costs (over years)

Core pattern: rate × time = accumulated quantity, where rate is constant
(or stepwise constant) over a known time interval.

**Group B — Remaining/Sequential Partition (8 cases)**
- Case 5: 20 students → 20% → 25% of remaining → rest
- Case 10: 60-mile trip, stop at 20, then stop 15 before end
- Case 12: 5 pies × 8 pieces, 18 pieces eaten, how many left
- Case 13: 80 notes, buy more, place 12, how many left
- Case 15: 30 lollipops, eat 2, package 2 per bag
- Case 16: Bridge 5000lb, truck weight + boxes
- Case 17: $40 order, 25% fee, $3 delivery
- Case 18: $500 material + $800 labor, 10% insurance

Core pattern: sequential operations on a starting quantity, where each
step subtracts or transforms a portion. The "remaining" at each step is
the state.

**Group C — Over-Year/Compound with Time Horizon (4 cases)**
- Case 19: 40-year pension, $50k/year, 5%/year after 20, quit after 25
- Case 20: $140/month, 10% less second half
- Case 23: $600/month salary, 10% increase every year
- Case 2: Drive 3h at 60mph, turn around, 2h traffic, remaining time
- Case 3: Month 1: 60, Month 2: 3×, Month 3: −30%

Core pattern: quantity changes over multiple time steps with different
rates or multipliers at each step.

**Group D — Time-Interval Arithmetic (2 cases)**
- Case 6: Hike, need avg speed 4 mph over remaining distance
- Case 8: Candle 2 cm/hour, 1 PM to 5 PM (calendar-clock)

**Group E — Mixtures/Spillage (2 cases)**
- Case 7: Orange + pineapple mix, spill 1 liter
- Case 24: n/a (only 24 total)

### Recommended Capability Design

I would recommend ONE capability with THREE supported forms:

**Form 1: ConstantRateOverInterval** (`r × t = Σ`)
- Inputs: rate per time unit, duration, optional initial offset
- Outputs: accumulated quantity at end of interval
- Safety: rate must be linear, interval must be monotonic
- Examples: Case 0, 1, 9, 11, 14, 21, 22, 4

**Form 2: SequentialStateTransition** (`S₀ → S₁ → S₂ → ...`)
- Inputs: initial state, ordered operations (subtract, partition, fee)
- Outputs: state at each step, final remaining
- Safety: operations must be deterministic at each step
- Examples: Case 5, 10, 12, 13, 15, 16, 17, 18

**Form 3: MultiStepTimeHorizon** (`t₁: r₁, t₂: r₂, ...`)
- Inputs: segmented time intervals with different rates per segment
- Outputs: accumulated values at interval boundaries
- Safety: intervals must be sequential and non-overlapping
- Examples: Case 19, 20, 23, 2, 3

**Ambiguity causes** (to recognize):
- Missing start time / reference point
- Ambiguous "over" (duration vs division vs spatial)
- Inclusive vs exclusive interval boundaries
- Partial time period (e.g., "half-year")
- Unspecified ordering when multiple events

**Must exclude**:
- Probability/likelihood (ForbidsLikelihoodSemantics)
- Compound growth beyond simple segmented rates (ForbidsRepeatedTemporalApplication)
- Calendar date arithmetic (DifferentMonthLengths, LeapYear)
- Causal inference from sequence (CorrelationNotCausation)
- Pure finance without temporal dimension (ForbidsFinancialConstructs)
- Abstract recurrence (back to Recurrence capability)

---

## Integrity Conditions

1. **No proposer modification**: All proposer logic and thresholds are frozen
   at the commit hash below. No changes will be made to the proposer after
   the holdout is executed.

2. **Single execution**: The holdout will be executed exactly once. If the
   run fails for infrastructure reasons (disk full, crash) it may be retried,
   but any retry must use identical proposer code.

3. **Full output preservation**: The proposal JSON, decision matrix, evidence
   hashes, and this rubric will be saved as a timestamped record.

4. **Transparent scoring**: Each rubric dimension will be scored with
   supporting evidence from the proposal output. No after-the-fact adjustment
   of criteria.

5. **No outcome shopping**: The outcome category will be assigned by the rubric
   rules, not by how "interesting" the result looks.

---

*Signed: pre-registered 2026-07-25 before any holdout execution.*
