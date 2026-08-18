# Stage 241 — shifted portfolio holdout

This stage evaluates the three selected source modules on a held-out language
surface using aliases, reordered explanatory clauses, and alternate target
verbs rather than formula identifiers alone.

Results:

* 180/180 shifted cases authorized and classified exactly;
* 540/540 frontend reports replayed and tamper checks rejected altered reports;
* 9/9 boundary cases refused, with 27/27 boundary replays and tamper checks;
* boundaries covered competing targets, missing inputs, and unsupported
  approximate/continuous requests;
* zero route leakage, false authorizations, or false denials;
* curriculum manifest unchanged and no live mutation.

The conditional replay denominator is explicit: each case was offered to all
three selected modules, so 180 cases generated 540 frontend receipts.

Holdout hash:
`8f0994361fb9a495e31752049564723f2e27f9a8b0292c18e62111de56eb62bf`
