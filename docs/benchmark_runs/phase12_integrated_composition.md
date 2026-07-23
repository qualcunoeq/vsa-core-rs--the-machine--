# Phase 12 — Unified mixed/composition integration

The focused composition corpus is now run beside the original blind mixed
vertical corpus in one release-mode process. This catches regressions where a
new composition path could steal an ordinary single-vertical case or alter a
fallback decision.

```bash
cargo run --release --quiet --bin integrated_bench \
  data/mixed_ood_v1.json data/cross_vertical_ood_v1.json \
  data/compositional_planner_ood_v1.json \
  data/raw_decomposition_ood_v1.json
```

Observed result:

```text
integrated cases: 2340 (mixed 1000 + composition 340 + planner 500 + raw 500)
mixed route: 1.000
mixed false authorizations: 0
mixed false denials: 0
mixed rewrite regressions: 0
composition authorized: 300
composition false authorizations: 0
composition false denials: 0
composition intermediate replay: 300
composition final replay: 300
composition rewrite regressions: 0
planner correct decisions: 500/500
planner authorized routes: 465
planner ambiguous routes: 10
planner false authorizations: 0
planner false denials: 0
planner replayed stages: 840
raw decomposition structural accuracy: 500/500
raw decomposition false authorizations: 0
raw decomposition false denials: 0
```

The composition unit test also tampers with both the first and second
intermediate boundaries; replay rejects both receipt mutations. The unified
run does not yet add a third executor stage or proposition-to-algebra proof
obligations. Those remain a separate scope expansion after this integration
baseline is stable.
