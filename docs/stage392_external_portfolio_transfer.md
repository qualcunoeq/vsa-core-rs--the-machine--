# Stage 392 — external portfolio transfer checkpoint

The frozen route-blind shadow portfolio was rerun against the independently
sourced 4,000-question release at commit 1e2c0da.

Development (3,000 questions):

- all seven routes were invoked: 21,000 route observations;
- 11 unique shadow candidates, 2,989 refusals, no route ambiguity;
- 11/11 candidate answers matched the development answer-hash oracle;
- 11/11 candidate executions replayed;
- zero incorrect candidates, false authorizations, or live mutation.

Sealed holdout (1,000 questions, privileged hash-only scoring):

- one unique shadow candidate and 999 refusals;
- 1/1 candidate matched the sealed answer-hash oracle and replayed;
- plaintext answers read: 0;
- sealed questions read for evaluation: 1,000;
- zero false authorizations and no live mutation.

This is the first current-commit external transfer signal, but it is small:
1/3,000 development and 1/1,000 sealed. The general router baseline remains
0/3,000 and 0/1,000. The portfolio remains shadow-only and is not promoted
from this result.
