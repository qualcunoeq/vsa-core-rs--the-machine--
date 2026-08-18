# Stage 240 — portfolio promotion and rollback

This stage exercises the lifecycle after utility selection and source
validation. Three validated portfolio modules are staged into a cloned
versioned registry, while the parent registry and curriculum manifest remain
untouched.

Results:

* 6 modules / 21 records and 120 typed gap observations;
* 3 selected modules at expected utility 200 and acquisition cost 10/10;
* 3/3 source-validation gates passed;
* 3/3 promotion attempts succeeded in the clone;
* 5/5 promotion receipts replayed and 5/5 tampered receipts rejected;
* an induced regression was blocked without changing the active version;
* a later revision rolled back successfully;
* world-state preservation and historical replay: 1/1 each;
* parent registry remained at zero versions while the clone held five;
* curriculum manifest unchanged;
* zero false authorizations, false denials, or live mutations.

Promotion is policy-gated and clone-only. The induced regression and rollback
are included to verify that accumulated world-state identity survives version
changes and that the parent/live registry cannot be mutated by the campaign.
