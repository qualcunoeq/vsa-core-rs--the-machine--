# Phase 12 — Unified mixed/composition integration

The focused composition corpus is now run beside the original blind mixed
vertical corpus in one release-mode process. This catches regressions where a
new composition path could steal an ordinary single-vertical case or alter a
fallback decision.

```bash
cargo run --release --quiet --bin integrated_bench \
  data/mixed_ood_v1.json data/cross_vertical_ood_v1.json
```

Observed result:

```text
integrated cases: 1340 (mixed 1000 + composition 340)
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
```

The composition unit test also tampers with both the first and second
intermediate boundaries; replay rejects both receipt mutations. The unified
run does not yet add a third executor stage or proposition-to-algebra proof
obligations. Those remain a separate scope expansion after this integration
baseline is stable.
