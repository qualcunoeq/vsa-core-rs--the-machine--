# Phase 23 — blinded technical-notation family audit

Phase 22 found 338 apparent normalization-contamination cases, with 227
specialist-notation markers and 105 embedded-formula markers. This phase
performs a second, blinded audit of those rows. It uses only the question
text, broad source category, and Phase 22 mechanism marker. It does **not**
read expected answers, terminal classifications, answer keys, or parser
state, and it never mutates the router, ontology, registry, or executor.

Run:

```text
cargo run --bin hle_notation_audit -- \
  /tmp/hle_normalization_audit_2147e9e.json \
  /tmp/hle_notation_audit_2147e9e.json
```

## Audit dimensions

Each retained case receives deterministic screening labels for:

* broad domain;
* notation family;
* presence of local symbol-definition cues;
* external-convention cues;
* likely interpretation status;
* likely downstream outlook;
* reasons and a conservative confidence marker.

The notation families are intentionally narrower than the Phase 22 marker:
equations/expressions, set logic/quantifiers, linear algebra/matrices,
probability/statistics/information, differential/dynamical systems,
geometry/topology, chemical structure, biological/medical nomenclature,
formal language/code, game/diagram notation, specialized named notation, and
mixed/unknown.

## Current shadow result

The report is generated from the immutable Phase 22 JSON artifact. The exact
input and source-trace hashes are retained in the report and must be copied
into the release record after each run. The audit is a screening instrument,
not a semantic oracle: a “likely unique” label does not authorize parsing, and
an external-convention label does not prove that retrieval would solve the
question.

The current run retained 332 rows in scope: 227 specialist-notation markers
and 105 embedded-formula markers. The six smaller Phase 22 markers are not
silently relabeled; they remain outside this notation-family report. The
blinded screening distribution was:

| Dimension | Largest observed groups |
|---|---|
| Domain | mathematics 221, biology/medicine 33, physics 27, statistics/information 27 |
| Family | equations/expressions 75, set logic/quantifiers 63, linear algebra/matrices 50, mixed/unknown 53 |
| Interpretation | locally defined but needs review 220, likely unique 59, ambiguous/malformed 32, visual 18 |
| Downstream outlook | likely normalization-only 263, manual review 32, visual/external 18, likely knowledge/reasoning gap 19 |

These counts are deterministic screening signals, not validated semantic
labels. In particular, local-definition cues are intentionally broad and can
occur in questions whose real difficulty is theorem knowledge or reasoning.
The next contract should therefore be selected from a manually reviewed,
homogeneous subset rather than from the largest aggregate family.

The next implementation should select one homogeneous family—preferably
locally defined mathematical notation with inline/display formulas—and build
an independent positive/ambiguous/unsupported corpus before changing the
parser. External domain conventions and visually essential layouts remain
outside the first contract.
