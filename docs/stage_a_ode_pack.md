# Stage A: bounded exact ODE pack

The curriculum now includes a shadow-only scalar ODE substrate. It accepts
constant-derivative equations and autonomous affine linear equations with an
explicit initial value and a bounded evaluation time. Constant-rate solutions
are emitted as exact rational artifacts; nonzero affine rates retain the
exponential term symbolically rather than approximating it numerically.

The pack refuses nonlinear equations, coupled systems, numerical
approximation, missing or inferred initial conditions, and horizons beyond
the declared bound. Every result carries assumptions, provenance, and a
replay hash.

The independent corpus is recorded in `docs/stage_a_ode_pack.json` with
SHA-256 `6948c7f08a9c197d6a3d8d483eca26f53076ce9eeebfbefc1899464e18743350`.

| outcome | cases |
| --- | ---: |
| supported | 120 |
| ambiguous | 40 |
| refused | 80 |
| exact decisions | 240/240 |
| supported artifacts | 120/120 |
| replay verified | 240/240 |
| tamper rejected | 240/240 |
| false authorizations | 0 |
| false denials | 0 |

The ODE pack remains shadow validated and does not alter production routing or
the frozen HLE holdout.
