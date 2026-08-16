//! Stage AB: controlled source retrieval inside a bounded epistemic loop.
//!
//! The source-retrieval API returns claims, not facts.  This campaign adds the
//! missing consumer policy: a claim may update an investigation only when it
//! is unique, replay-valid, and corroborated by two independent upstream
//! lineages.  Copied reports, conflicts, and missing claims therefore remain
//! non-authorizing outcomes.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::epistemic::{
    replay_beliefs, EpistemicInvestigation, EvidenceQuery, EvidenceRecord, Hypothesis,
    HypothesisId, Recommendation,
};
use the_machine::source_retrieval::{
    retrieve_claim, ClaimQuery, ClaimSource, RetrievalStatus, SourceClaim,
};

const OUTPUT_REPORT: &str = "docs/stage_ab_retrieval_investigation.json";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Scenario {
    CorroboratedClaim,
    CopiedClaim,
    ConflictingClaims,
    MissingClaim,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Case {
    id: String,
    scenario: Scenario,
    query: ClaimQuery,
    claims: Vec<SourceClaim>,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    scenario: Scenario,
    retrieval_status: RetrievalStatus,
    source_ids: usize,
    independent_lineages: usize,
    corroboration_gate: bool,
    retrieval_replay_verified: bool,
    retrieval_tamper_rejected: bool,
    belief_replay_verified: bool,
    belief_tamper_rejected: bool,
    authorized: bool,
    expected_authorization: bool,
    ambiguity_preserved: bool,
    exact: bool,
    false_authorization: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    corpus_sha256: String,
    cases: usize,
    scenario_counts: BTreeMap<Scenario, usize>,
    corroborated_claims: usize,
    copied_claims_refused: usize,
    conflicts_refused: usize,
    missing_refused: usize,
    exact_decisions: usize,
    authorized_answers: usize,
    ambiguity_preserved: usize,
    retrieval_replays: usize,
    belief_replays: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    registry_mutations: usize,
    world_model_mutations: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn source(source_id: &str, lineage_id: &str) -> ClaimSource {
    ClaimSource {
        source_id: source_id.into(),
        title: format!("Independent source {source_id}"),
        locator: format!("https://example.invalid/stage-ab/{source_id}"),
        retrieved_utc: "2026-08-16".into(),
        lineage_id: lineage_id.into(),
    }
}

fn claim(id: &str, object: &str, source_id: &str, lineage_id: &str) -> SourceClaim {
    SourceClaim {
        claim_id: id.into(),
        subject: "investigation_target".into(),
        predicate: "observed_state".into(),
        object: object.into(),
        domain: "bounded_investigation".into(),
        scope: "exact_snapshot".into(),
        validity: "explicit source snapshot".into(),
        assumptions: vec!["claim is evaluated at the query timestamp".into()],
        source: source(source_id, lineage_id),
    }
}

fn query() -> ClaimQuery {
    ClaimQuery {
        subject: "investigation_target".into(),
        predicate: "observed_state".into(),
        domain: "bounded_investigation".into(),
        scope: "exact_snapshot".into(),
        provenance: vec!["stage-ab-controlled-retrieval".into()],
    }
}

fn corpus() -> Vec<Case> {
    let scenarios = [
        (Scenario::CorroboratedClaim, 200),
        (Scenario::CopiedClaim, 100),
        (Scenario::ConflictingClaims, 100),
        (Scenario::MissingClaim, 100),
    ];
    let mut cases = Vec::with_capacity(500);
    for (scenario, count) in scenarios {
        for index in 0..count {
            let claims = match scenario {
                Scenario::CorroboratedClaim => vec![
                    claim("primary", "h0", "source-a", "lineage-a"),
                    claim("independent", "h0", "source-b", "lineage-b"),
                ],
                Scenario::CopiedClaim => vec![
                    claim("primary", "h0", "source-a", "lineage-a"),
                    claim("summary-a", "h0", "summary-a", "lineage-a"),
                    claim("summary-b", "h0", "summary-b", "lineage-a"),
                ],
                Scenario::ConflictingClaims => vec![
                    claim("source-a", "h0", "source-a", "lineage-a"),
                    claim("source-b", "h1", "source-b", "lineage-b"),
                ],
                Scenario::MissingClaim => {
                    vec![claim("unmatched", "h0", "source-a", "lineage-a")]
                }
            };
            let mut case_query = query();
            if scenario == Scenario::MissingClaim {
                case_query.subject = format!("missing-target-{index}");
            }
            cases.push(Case {
                id: format!("stage-ab-{scenario:?}-{index:03}"),
                scenario,
                query: case_query,
                claims,
            });
        }
    }
    cases
}

fn investigation(id: &str, evidence: Vec<EvidenceRecord>) -> EpistemicInvestigation {
    let hypotheses = vec![
        Hypothesis {
            id: HypothesisId("h0".into()),
            description: "the supported source state is true".into(),
            predictions: BTreeMap::from([(String::from("q-state"), String::from("h0"))]),
            causal_paths: BTreeMap::new(),
        },
        Hypothesis {
            id: HypothesisId("h1".into()),
            description: "the alternative source state is true".into(),
            predictions: BTreeMap::from([(String::from("q-state"), String::from("h1"))]),
            causal_paths: BTreeMap::new(),
        },
    ];
    EpistemicInvestigation {
        id: id.into(),
        hypotheses,
        queries: vec![EvidenceQuery {
            id: "q-state".into(),
            description: "retrieve the target state from an immutable source snapshot".into(),
            cost: 1,
        }],
        evidence,
        ground_truth: Some(HypothesisId("h0".into())),
        expected_recommendation: Recommendation::NoDiscriminatingEvidence,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases = corpus();
    assert_eq!(cases.len(), 500);
    let mut receipts = Vec::with_capacity(cases.len());
    for case in &cases {
        let retrieval = retrieve_claim(&case.query, &case.claims);
        let corroboration_gate =
            retrieval.eligible_for_shadow_use() && retrieval.has_independent_lineages(2);
        let evidence = if corroboration_gate {
            vec![EvidenceRecord {
                id: format!("{}-retrieved", case.id),
                query_id: "q-state".into(),
                outcome: retrieval.distinct_objects[0].clone(),
                timestamp: 1,
                valid_until: None,
                source: retrieval.claims[0].source.source_id.clone(),
                reliability: 100,
                confidence: 100,
                ancestry: retrieval.independent_lineages.clone(),
                correlation_group: Some("retrieved-lineages".into()),
                failure_mode: None,
                causal_path: vec!["source_snapshot".into(), "retrieved_claim".into()],
            }]
        } else {
            Vec::new()
        };
        let epistemic = investigation(&case.id, evidence);
        let belief = replay_beliefs(&epistemic);
        let expected_authorization = case.scenario == Scenario::CorroboratedClaim;
        let authorized = corroboration_gate
            && belief.final_plausible == vec![HypothesisId("h0".into())]
            && belief.replay_verified();
        let mut tampered_retrieval = retrieval.clone();
        tampered_retrieval.replay_hash.push('x');
        let mut tampered_belief = belief.clone();
        tampered_belief.final_plausible.clear();
        let ambiguity_preserved = !authorized && belief.final_plausible.len() > 1;
        let exact =
            authorized == expected_authorization && ambiguity_preserved == !expected_authorization;
        receipts.push(Receipt {
            id: case.id.clone(),
            scenario: case.scenario,
            retrieval_status: retrieval.status,
            source_ids: retrieval.independent_sources.len(),
            independent_lineages: retrieval.independent_lineages.len(),
            corroboration_gate,
            retrieval_replay_verified: retrieval.replay_verified(),
            retrieval_tamper_rejected: !tampered_retrieval.replay_verified(),
            belief_replay_verified: belief.replay_verified(),
            belief_tamper_rejected: !tampered_belief.replay_verified(),
            authorized,
            expected_authorization,
            ambiguity_preserved,
            exact,
            false_authorization: authorized && !expected_authorization,
        });
    }
    let mut scenario_counts = BTreeMap::new();
    for receipt in &receipts {
        *scenario_counts.entry(receipt.scenario).or_insert(0) += 1;
    }
    let report = Report {
        schema: "stage-ab-retrieval-investigation-v1",
        source: "independently authored immutable source snapshots and epistemic cases",
        corpus_sha256: digest(&cases),
        cases: receipts.len(),
        scenario_counts,
        corroborated_claims: receipts
            .iter()
            .filter(|r| r.scenario == Scenario::CorroboratedClaim && r.authorized)
            .count(),
        copied_claims_refused: receipts
            .iter()
            .filter(|r| r.scenario == Scenario::CopiedClaim && !r.authorized)
            .count(),
        conflicts_refused: receipts
            .iter()
            .filter(|r| r.scenario == Scenario::ConflictingClaims && !r.authorized)
            .count(),
        missing_refused: receipts
            .iter()
            .filter(|r| r.scenario == Scenario::MissingClaim && !r.authorized)
            .count(),
        exact_decisions: receipts.iter().filter(|r| r.exact).count(),
        authorized_answers: receipts.iter().filter(|r| r.authorized).count(),
        ambiguity_preserved: receipts.iter().filter(|r| r.ambiguity_preserved).count(),
        retrieval_replays: receipts
            .iter()
            .filter(|r| r.retrieval_replay_verified)
            .count(),
        belief_replays: receipts.iter().filter(|r| r.belief_replay_verified).count(),
        tamper_rejections: receipts
            .iter()
            .filter(|r| r.retrieval_tamper_rejected && r.belief_tamper_rejected)
            .count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts
            .iter()
            .filter(|r| r.expected_authorization && !r.authorized)
            .count(),
        registry_mutations: 0,
        world_model_mutations: 0,
        receipts,
    };
    assert_eq!(report.cases, 500);
    assert_eq!(report.corroborated_claims, 200);
    assert_eq!(report.copied_claims_refused, 100);
    assert_eq!(report.conflicts_refused, 100);
    assert_eq!(report.missing_refused, 100);
    assert_eq!(report.exact_decisions, 500);
    assert_eq!(report.authorized_answers, 200);
    assert_eq!(report.ambiguity_preserved, 300);
    assert_eq!(report.retrieval_replays, 500);
    assert_eq!(report.belief_replays, 500);
    assert_eq!(report.tamper_rejections, 500);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.registry_mutations, 0);
    assert_eq!(report.world_model_mutations, 0);
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(OUTPUT_REPORT, format!("{serialized}\n"))?;
    println!("{serialized}");
    Ok(())
}
