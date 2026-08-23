# Stage 453 — source-derived complex arithmetic portfolio route

This development-only checkpoint integrates the previously pressure-tested
source-derived rectangular complex-arithmetic pack into the route-blind Goal 6
portfolio.  The route emits an exact scalar or complex-pair artifact only when
the frontend and source-derived evaluator both complete and replay.

| Metric | Result |
|---|---:|
| Development questions / answer hashes | 3000 / 3000 |
| Route invocations | 51000 (17 routes × 3000 questions) |
| Unique shadow candidates | 26 |
| Correct candidate hashes | 26 |
| Candidate replay verification | 26/26 |
| Multiple-route ambiguities / no route | 0 / 2974 |
| Production authorizations / false authorizations | 0 / 0 |
| Manifest unchanged | true |

The complex route produced one development candidate, but it overlapped an
existing executable route and therefore did not increase unique development
coverage.  It remained route-safe: all candidates matched their development
hashes and no route ambiguity was introduced.

Evidence:

* implementation under test: working-tree route extension after `17a3649`
* dataset SHA-256: `3cf924116a0f8f6a0c84d0ce7949b0c1e16221e0d4b5fcb0c4322110e30714f2`
* probe report SHA-256: `1fef4bd8244f7e0f45ce79de5aae28eb4e616993c55d83d71f5790a32bccabdf`
* score report SHA-256: `79891414d09e08faefcfcd9302430ad30b743b3146a46f9de9f8380465b5c74f`

The underlying source-derived complex pack was independently pressure-tested
at Stage 408 before this route integration.
