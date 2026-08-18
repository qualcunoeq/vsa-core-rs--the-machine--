# Stage 222 — versioned source-catalog memory route

This gate connects source-derived formula catalogs to append-only curriculum
memory and the operative report frontend. Five catalog versions are admitted
to a clone at `v2`; each case retrieves by exact domain, artifact type, and
version before language grounding. A missing version or a duplicate exact
version is never replaced by a nearby catalog.

Results:

* 500 cases: 300 supported, 100 ambiguous, 100 missing-version;
* 500/500 exact terminal decisions and 300/300 authorized routes;
* 2,500/2,500 exact catalog lookups replayed and tamper-rejected;
* 25 duplicate-version catalog conflicts and 500 missing-version refusals;
* 1,975 report frontends replayed with provenance preserved;
* 300/300 downstream replays and tamper rejections;
* all 500 clone memories unchanged;
* zero false authorizations, false denials, or live memory mutations.

Corpus hash:
`7546cc236556eb2e9e0c429c87229f54f7fbedf9a3eab13d9aa685426f454ff4`.

The operation is shadow-only. Source records, current versions, and conflict
versions remain immutable artifacts; production routing and the live registry
are unchanged.
