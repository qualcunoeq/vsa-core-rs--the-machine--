# Stage R: source frontend route-blind language

This gate invokes the existing source-specific frontends; it does not construct typed requests directly from benchmark markers.

- Cases: 800 (480 supported, 160 ambiguous, 160 unsupported)
- Exact decisions: 800/800
- Frontend-complete and typed requests: 480/480
- Source provenance preserved: 480/480 authorized artifacts
- Replay verified: 800/800
- Tamper rejected: 800/800
- False authorizations / denials: 0 / 0
- Multi-module ambiguity candidates: 0
- HLE questions read: 0
- Production registry mutations: 0
- Corpus report: `docs/stage_r_source_frontend_route_blind.json`
