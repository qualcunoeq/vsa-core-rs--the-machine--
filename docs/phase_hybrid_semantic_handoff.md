# Hybrid semantic handoff

Validated semantic proposals can now lower into the existing
`EquationProblemBindingV1` representation without selecting a solver.

The handoff repeats IR validation, requires relation-symbol closure, preserves
source spans/scopes/assumptions, and finalizes the binding replay hash through a
trusted constructor. Property, proof, operator, and unknown targets are not
forced through an equation interface; they remain explicitly unsupported until
typed consumers exist.

Every handoff is non-authorizing. It cannot produce an answer, mutate a pack,
change a registry, or update the world model. The handoff receipt records the
candidate hash, diagnostics, binding (when complete), and its own replay hash.

Focused tests cover complete equation lowering, refusal of property targets,
and rejection of unbound relation symbols.
