# Stage 96 — shifted technical-language extension

This gate puts a larger independently generated language surface in front of
the two newly acquired source-derived domains. It includes reordered clauses,
paraphrased target verbs, parenthesized endpoint notation, `P(B|A)` notation,
irrelevant cross-domain mentions, explicit alternatives, missing evidence,
approximate/continuous near-misses, and unsupported models.

## Results

| metric | result |
|---|---:|
| cases | 800 |
| supported / ambiguous / unsupported | 480 / 160 / 160 |
| exact terminal classifications | 800/800 |
| authorized supported routes | 480/480 |
| ambiguity preserved | 160/160 |
| unsupported refused | 160/160 |
| frontend and downstream replay | 800/800 |
| tamper rejection | 800/800 |
| provenance preserved | 800/800 |
| false authorizations / denials | 0 / 0 |

The permanent partitions are 480 development, 160 validation, and 160 sealed
cases, each containing both source domains and all three outcome classes. The
full receipt artifact is [stage96_source_language_extension.json](stage96_source_language_extension.json).

This is a technical-language transfer gate, not an HLE claim. The frontends
remain bounded: they require explicit quantities and reject unsupported
approximations, missing evidence, ambiguous operations, and domain-specific
semantics that are not represented in the source catalogs.
