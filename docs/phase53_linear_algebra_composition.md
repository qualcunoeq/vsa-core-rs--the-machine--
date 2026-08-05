# Phase 53 — Linear-algebra cross-domain composition

Phase 53 tests the linear-algebra pack as a typed substrate rather than an
isolated calculator. The corpus intentionally includes consumers that exist,
consumers that are not yet authorized, and a domain that is explicitly outside
the pack.

## Composition corpus

The 80 cases contain:

* 20 matrix → existing 2×2 linear-system routes;
* 20 matrix → bounded recurrence-style transition-vector routes;
* 20 graph-adjacency candidates with no graph consumer yet;
* 10 covariance-shaped candidates with no probability consumer yet;
* 10 parameterized matrices outside the exact-integer domain.

Results (`docs/phase53_linear_algebra_composition.json`):

* 80/80 route decisions exact;
* 80/80 intermediate matrix artifacts replayed;
* 40/40 downstream routes replayed;
* 40 unsupported graph/probability/domain paths safely refused;
* 0 route leakage;
* 0 false authorizations.

The graph and covariance cases remain typed candidates, not silently promoted
to graph-theory or probability semantics. This keeps the next curriculum pack
boundary explicit.

The linear-algebra curriculum item remains `shadow_validated`; HLE is still a
deferred frozen checkpoint.
