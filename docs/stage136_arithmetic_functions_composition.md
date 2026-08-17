# Stage 136 — arithmetic-functions cross-domain composition

The new bounded arithmetic-functions pack now participates in the curriculum
through explicit typed handoffs. The campaign does not treat an arithmetic
scalar as semantically self-describing:

* a divisor certificate's input value may feed a bounded Euler-totient request;
* divisor count and sum may feed a declared Bézout prerequisite and modular
  inverse;
* a prime-counting result may become a combinatorial population size only when
  that role is explicit;
* a Möbius input value may feed a bounded modular-ring construction, while its
  signed value is not treated as a probability.

| Measure | Result |
|---|---:|
| Cases | 240 |
| Supported / ambiguous / refused | 120 / 40 / 80 |
| Exact route decisions | 240/240 |
| Supported typed handoffs | 120/120 |
| Replay verified | 240/240 |
| Tamper rejected | 240/240 |
| False authorizations / denials | 0 / 0 |

The boundary corpus executes real refusal paths for analytic or unbounded
requests, non-unit inverses, signed Möbius weights presented as probabilities,
and missing consumer roles. No registry, curriculum manifest, or production
router is mutated.

Corpus SHA-256: `9c601b07a0612d635f2b6bf74cf3ce1cf967b8bf366e7e36deb1ad82d3b55da2`

Reproduce with:

```text
cargo run --quiet --bin stage136_arithmetic_functions_composition
```
