# Stage 349 — generic multi-format source discovery

* source documents / discovered catalogs / records: 26 / 13 / 54
* catalog kinds (formula / relation / topology): 11 / 1 / 1
* unmarked / rejected declarative documents: 12 / 1
* structural cases / exact decisions: 216 / 216
* replay / tamper rejection: 216 / 216
* source mutations / rejected: 52 / 52
* catalog tamper rejections: 13
* false authorizations / denials: 0 / 0
* production mutations: 0
* source manifest: `43cb30f18ac7f1aad287abe60f36e4af254259385668e1065c2efefc21cc1db3`

The discovery path admits only one explicitly declared declarative format per document. Formula, relation, and finite-topology records are parsed by their existing provenance/schema gates and then represented by one generic catalog envelope. Unmarked material is retained as evidence, mixed or malformed documents are rejected, and all admitted catalogs are exercised with supported, missing, ambiguous, and invalid-domain requests. No subject-specific routing branch or live registry mutation is used.
