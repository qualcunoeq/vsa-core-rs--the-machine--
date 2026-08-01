# Phase 35 — Classical mechanics pressure and repair benchmark

This is a shadow-only pressure campaign for the externally sourced Phase 34
classical-mechanics pack. It does not change the production registry, HLE
routing, or the parent knowledge pack.

## Reproducibility

```text
cargo run --quiet --bin classical_mechanics_pressure_bench -- \
  docs/phase35_classical_mechanics_pressure_bench.json
```

The deterministic corpus and report are committed in
[`phase35_classical_mechanics_pressure_bench.json`](phase35_classical_mechanics_pressure_bench.json).
The report hash is:

```text
0dc80067f9e7f11a1bc24353d5528b981525f7a388ad169e309d114f425d02dd
```

The benchmark uses an independent reference oracle for the five supported
relations. It includes 240 cases:

* 160 supported numerical exercises across Newton's law, momentum, kinetic
  energy, Hooke force, and elastic potential energy;
* 20 ambiguous energy-alias cases;
* 20 incompatible-unit cases;
* 10 missing-binding cases;
* 10 unsupported-domain cases;
* 20 unsupported multi-law-composition cases.

The pressure cases cover direct inverse calculations, signed values, alias
variation, missing quantities, unit mismatches, ambiguous targets, invalid
regimes, and composition attempts that the single-law pack must refuse.

## Results

| Metric | Result |
| --- | ---: |
| Total cases | 240/240 |
| Exact status decisions | 240/240 |
| Exact supported values | 160/160 |
| Replay receipts | 240/240 |
| False authorizations | 0 |
| False denials | 0 |
| Injected defect classes | 7/7 produced counterexamples |
| Defect diagnoses | 7/7 |
| Sandbox repairs | 7/7 |
| Parent pack immutable | yes |
| Registry mutated | no |

## Defect campaign

The sandbox models seven implementation defects:

* swapped Newton variables;
* omitted square in kinetic energy;
* incorrect Hooke-law sign;
* momentum/force confusion;
* ignored unit mismatch;
* bypassed domain assumptions;
* omitted replay verification.

Each mutation has at least one deterministic counterexample. The repair result
is evaluated with the canonical pack in an immutable sandbox clone; no defect
is applied to the stored parent pack. The revised shadow receipt records a
different revision fingerprint while retaining the original pack hash.

This phase validates pressure and repair of the existing domain pack. It does
not claim broad mechanics competence, natural-language mechanics ingestion, or
HLE improvement. The next gate is an independently authored textbook-style
corpus and only then a diagnostic HLE rerun.
