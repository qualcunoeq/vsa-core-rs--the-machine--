# Stage D — generic source-relation ingestion

The source-acquisition layer now supports explicit relation records in
attributed documents. The parser and validator are domain-agnostic: they
check block structure, IDs, aliases, scopes, pair uniqueness, assumptions,
and citation completeness. The executor performs only the declared lookup;
it contains no DNA-specific branch.

The source document contains one OpenStax-derived complementary-base relation
record. Its generic output was compared with the existing biology pack only
after both artifacts were independently replay-verified.

| Check | Result |
| --- | ---: |
| Extracted relation records | 1 |
| Valid catalogs | 1/1 |
| Mutated catalogs rejected | 6/6 |
| Independent exercises | 120 |
| Generic/biology agreement | 120/120 |
| Ambiguous cases preserved | 40 |
| Refused cases | 80 |
| Exact decisions | 240/240 |
| Replay verification | 240/240 |
| Tamper rejection | 240/240 |
| Source provenance preserved | 240/240 |
| False authorizations | 0 |

Source document SHA-256:
`9a34afa1464ee6290fc5026d3210457d50ba56fb0e70ca97bcbfb634b527bbc6`

Corpus SHA-256:
`4f23ef8940a57469dde59c0a0d7cd88fe6c2b447f48f02cf321f811e88c52b0d`

The extracted relation remains an immutable shadow artifact. It does not
mutate the biology pack, curriculum manifest, live router, or production
registry.

Reproduction:

```text
cargo run --quiet --bin source_relation_ingestion_bench
```
