# Stage H: source-derived science composition

This shadow-only campaign checks that exact kinetic-energy and Hooke-force
artifacts from the source-derived science pack agree with the independently
validated classical-mechanics pack. It also exercises ambiguous energy aliases
and refuses thermodynamic, unknown-law, and invalid-unit routes.

The benchmark contains 240 cases: 120 supported equivalence routes, 40
ambiguous routes, and 80 refusals. Each pack result is replay-checked and
tampered hashes are rejected. No composition route authorizes a result unless
both source semantics and the typed mechanics law are complete.

The generated JSON report is `docs/stage_h_science_composition.json`.
