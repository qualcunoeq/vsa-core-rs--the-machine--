# Phase 31 — PercentageQuantityV1 Implementation Contract

## Status

This contract establishes the boundary **before** implementation. The corpus
(`eee07ad`) is frozen and deterministic. No parser, executor, or routing change
is permitted until the full validation sequence completes.

The corpus has 350 cases with the following shape:

| Category       | Count | Purpose                                            |
|----------------|-------|----------------------------------------------------|
| Supported      | 200   | Canonical linear percentage semantics              |
| Ambiguous      | 50    | Missing base, direction, or interpretation         |
| Adversarial    | 100   | Scope-creep cases (compound, finance, points, etc) |
| Rewrite pairs  | 50    | Surface-variant preservation (25 of + 25 discount) |

Frozen contract-corpus hash (from `eee07ad`):
`f6ebdcf826e10125050f5b83b50a31a38fcb4b7f8c197dc6d0833a91bf7d336c`

## Supported Target Forms (V1)

V1 implements exactly **four** explicit target forms. No other arithmetic
engines are permitted.

### 1. PercentageOf

```
part = rate × base
```

Parsed from:
- "What is X% of Y?"
- "Find X percent of Y."
- "Calculate X percent of the whole quantity Y."

Rate is a percentage (already divided by 100 in the schema, preserved as a
percentage value in the artifact for interpretability). The reference base is
the whole quantity.

### 2. IncreaseByPercentage

```
final = base × (1 + rate)
```

Parsed from:
- "A quantity increases by X%."
- "Apply an X% markup to base Y."
- "Base Y grows by X% (single step, one change only)."

Discount/Markup/Tax are **semantic labels** over this form or DecreaseByPercent,
not separate arithmetic engines. The reference base and direction must be
explicit in the source.

### 3. DecreaseByPercentage

```
final = base × (1 − rate)
```

Parsed from:
- "A quantity decreases by X%."
- "Apply an X% discount to base Y."
- "Reduce Y by X%."

Same constraint: direction must be unambiguous in the surface form.

### 4. RecoverBase

```
base = final / (1 ± rate)
```

Parsed from:
- "After an X% increase, the new value is Y. What was the original?"
- "The discounted price is Y after an X% reduction. Find the original."

The sign matches the direction of the applied change. This is the inverse
operation — requires explicit statement that the given value is the *result*
of a single percentage change.

## Artifact Schema

Every V1 implementation must preserve more than the equation. The artifact
carries at least:

```
PercentageQuantityV1 {
    operation_kind:    PercentageOf | IncreaseByPercentage |
                       DecreaseByPercentage | RecoverBase
    base_quantity:     f64                       // reference/base quantity
    rate:              f64                       // percentage rate (as entered, e.g. 20 for 20%)
    direction:         Increase | Decrease        // only meaningful for change forms
    target_quantity:   f64                       // computed (or given for RecoverBase)
    single_step:       bool                      // MUST be true for V1
    provenance:        SourceSpan                // positions in source text
}
```

This makes replay capable of detecting the single most dangerous error: **using
the wrong reference base while still producing valid arithmetic.**

## Outside V1 (Explicitly Refused)

The following must **not** be supported, even when the surface resembles a
linear percentage relation:

- Sequential or compounded percentage changes
- Simple or compound interest
- Profit margin versus markup (requires separate relation)
- Percentage-point changes
- Probability
- Unspecified reference quantities
- Ambiguous "increased to/by" language
- Multiple discounts or tax-after-discount chains
- "Twice as likely" or similar non-percentage ratios

Ambiguous cases in the corpus (50) carry no executable schema. Adversarial
cases (100) are rejected at the classifier level.

## Validation Sequence

After the 350-case corpus passes (all supported cases carry correct typed
relation schemas, all ambiguous/adversarial cases carry none):

1. **Bridge** accepted artifacts into the exact algebra executor (the existing
   `QuantityRelation` graph, not a special-purpose percentage runtime).

2. **Tamper** the rate, reference base, direction, and target independently.
   Each permutation must produce a detectable artifact mismatch — not silently
   valid arithmetic on the wrong base.

3. **Compose** with `QuantityRelation` while forbidding implicit multi-period
   chains. Composition must preserve the single-step assumption.

4. **Reclassify** the frozen GSM8K release diagnostically — measure which
   residual errors are addressed, which are unrelated, and which are introduced.

5. **Rerun** the integrated suite (all planners, all governors, full QA,
   anomaly detection) before considering global routing changes.

## Success Criterion

The external taxonomy justified this capability. The eventual GSM8K gain is
secondary. The real success criterion is:

> **Whether PercentageQuantityV1 can add coverage while preserving the semantic
> differences between *of*, *by*, *to*, *more than*, and *less than*.**

If V1 collapses these distinctions, it is a regression regardless of benchmark
score.
