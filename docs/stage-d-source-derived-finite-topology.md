# Stage D — source-derived bounded finite topology

This is the first new mathematical domain acquired through the source
pipeline rather than reusing an existing subject pack. The source record is
extracted from *Topology Without Tears*, Definition 1.3.1, and carries its
citation, evidence span, validity bound, and axioms. The executor is a
generic finite-set engine; it validates a declared topology and computes
open-set, closed-set, interior, and closure artifacts only for carriers of at
most eight named points.

## Independent shadow campaign

The benchmark contains 240 cases generated independently of the source
transcription: 120 supported, 40 ambiguous, and 80 unsupported. Supported
cases cover topology validation, open and closed membership, interior, and
closure. Boundaries cover metric/infinite domains, oversized carriers,
invalid open-set families, and missing target sets.

| metric | result |
| --- | ---: |
| extracted source records | 1 |
| source mutations rejected | 6/6 |
| cases | 240 |
| supported artifacts | 120/120 |
| ambiguous decisions | 40/40 |
| unsupported decisions | 80/80 |
| exact decisions | 240/240 |
| replay verification | 240/240 |
| tamper rejection | 240/240 |
| false authorizations | 0 |
| false denials | 0 |
| production authorizations | 0 |
| curriculum manifest unchanged during execution | yes |

The machine-readable report is
[`stage-d-source-derived-finite-topology.json`](stage-d-source-derived-finite-topology.json).
The source document is
[`topology_without_tears_finite_definition.txt`](sources/topology_without_tears_finite_definition.txt).
The domain is recorded as `shadow_validated` in the curriculum manifest but
is not live-routed or promoted.
