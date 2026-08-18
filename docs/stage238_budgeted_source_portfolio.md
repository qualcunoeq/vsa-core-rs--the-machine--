# Stage 238 — budgeted source portfolio selection

Stage 237 ranked utility-guided proposals and acquired a fixed prefix. This
stage adds a deterministic budgeted portfolio gate and verifies that source
selection is utility-maximizing rather than prefix-based.

Results:

* 6 modules and 21 provenance-derived records discovered;
* 120/120 gap observations replayed across 6 exact clusters;
* 7/7 proposals replayed;
* the budgeted portfolio receipt replayed and its tampered receipt rejected;
* a cost budget of 10 selected 3 modules with expected utility 200 and total
  cost exactly 10;
* 600/600 source cases classified exactly;
* 300/300 selected cases authorized and 300/300 unselected cases refused;
* 1,800/1,800 frontend replays and tamper rejections;
* 300/300 downstream replays;
* 3/3 selected catalogs appended, uniquely retrieved, and replayed in a clone;
* parent memory and curriculum manifest unchanged;
* zero false authorizations, false denials, or live mutations.

Only replay-valid proposals already marked `Proposed` by the base planner are
eligible. Utility cannot bypass source authority, exact overlap, prerequisite,
cost, replay, or immutable-clone gates.

Corpus hash:
`c307c33cafabacd4596480d2eb5f3e05c1a5042f631263311c0a51fb4c706144`
