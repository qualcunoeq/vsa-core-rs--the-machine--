# Stage M — sandbox source ingestion and exercise validation

This gate connects a selected continuous-education plan to the existing
declarative source pipeline without promoting it. A source document is parsed
into cited formula records, schema-validated, executed by the generic
interpreter, and tested on independent supported and boundary exercises.

Reproduce with:

```text
cargo run --quiet --bin stage_m_source_validation
```

| Measure | Result |
|---|---:|
| Source records | 2 |
| Catalog valid | true |
| Independent exercises | 40/40 complete |
| Exercise replay | 40/40 |
| Exercise tamper rejection | 40/40 |
| Boundary cases refused | 10/10 |
| Boundary replay | 10/10 |
| Boundary tamper rejection | 10/10 |
| Source provenance preserved | true |
| Generic validation status | validated |
| Validation receipt replay | true |
| Validation tamper rejection | true |
| Mutated validation rejected | true |
| Candidates admitted after validation | 1 |
| Validated campaign cases resolved | 1 |
| Validated campaign replay | true |
| Validated campaign manifest unchanged | true |
| Curriculum manifest unchanged | true |
| False authorizations | 0 |

The validator is subject-neutral: it checks provenance, source identity,
independent exercise coverage, boundary refusal, replay, and tamper evidence.
It does not add a formula-specific branch or promote the module into live
routing.

Hashes:

* source document:
  `10baa036b588946a010d37ec4f27f366dc10f7f431a6abbb27774ee92a8b90f6`
* campaign corpus:
  `9c48ce7357c2aaee3da864a2c43a0250b60ae4f731d372e9344c8985e09aa5ce`

This completes the first source-backed sandbox-validation handoff for the
continuous education planner. Promotion and live routing remain separate
policy gates.
