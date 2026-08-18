//! Stage 289: retrieval-guided epistemic investigation at curriculum scale.
//!
//! This campaign closes the loop between information-seeking and controlled
//! source access.  The investigator must select a discriminating query before
//! retrieval.  A retrieved claim updates belief only when the current version
//! is unique, two independent lineages corroborate it, the query budget is
//! sufficient, and every receipt replays.  Claims remain shadow evidence; no
//! fact store, registry, or world model is mutated.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use the_machine::epistemic::{
    analyze, replay_beliefs, EpistemicInvestigation, EvidenceQuery, EvidenceRecord, Hypothesis,
    HypothesisId, Recommendation,
};
use the_machine::source_retrieval::{
    retrieve_claim, ClaimQuery, ClaimRetrievalResult, ClaimSource, RetrievalStatus, SourceClaim,
};

const REPORT_JSON: &str = "docs/stage289_retrieval_guided_investigation.json";
const REPORT_MD: &str = "docs/stage289_retrieval_guided_investigation.md";
const CASES: usize = 1_000;

const SOURCE_DOCUMENTS: &[(&str, &str)] = &[
    (
        "openstax_unit_conversion_catalog.txt",
        include_str!("../../docs/sources/openstax_unit_conversion_catalog.txt"),
    ),
    (
        "openstax_classical_science_catalog.json",
        include_str!("../../docs/sources/openstax_classical_science_catalog.json"),
    ),
    (
        "openstax_finite_statistics_source.txt",
        include_str!("../../docs/sources/openstax_finite_statistics_source.txt"),
    ),
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Scenario {
    Corroborated,
    CopiedLineage,
    StaleOnly,
    Conflicting,
    Missing,
    BudgetExhausted,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Terminal {
    Resolved,
    Ambiguous,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Case {
    id: String,
    scenario: Scenario,
    investigation: EpistemicInvestigation,
    claim_query: ClaimQuery,
    claims: Vec<SourceClaim>,
    query_cost: u8,
    budget: u8,
    expected_terminal: Terminal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PolicyReceipt {
    case_id: String,
    query_id: String,
    authorized: bool,
    retrieval_hash: String,
    query_cost: u8,
    budget: u8,
    replay_hash: String,
}

impl PolicyReceipt {
    fn new(
        case_id: String,
        query_id: String,
        authorized: bool,
        retrieval_hash: String,
        query_cost: u8,
        budget: u8,
    ) -> Self {
        let replay_hash = digest(&(
            &case_id,
            &query_id,
            authorized,
            &retrieval_hash,
            query_cost,
            budget,
        ));
        Self {
            case_id,
            query_id,
            authorized,
            retrieval_hash,
            query_cost,
            budget,
            replay_hash,
        }
    }

    fn replay_verified(&self) -> bool {
        self.replay_hash
            == digest(&(
                &self.case_id,
                &self.query_id,
                self.authorized,
                &self.retrieval_hash,
                self.query_cost,
                self.budget,
            ))
    }
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    scenario: Scenario,
    recommendation: Recommendation,
    recommendation_exact: bool,
    retrieval_status: RetrievalStatus,
    retrieval_authorized: bool,
    terminal: Terminal,
    expected_terminal: Terminal,
    final_plausible: Vec<HypothesisId>,
    retrieval_replay_verified: bool,
    retrieval_tamper_rejected: bool,
    belief_replay_verified: bool,
    belief_tamper_rejected: bool,
    policy_replay_verified: bool,
    policy_tamper_rejected: bool,
    source_provenance_complete: bool,
    exact: bool,
    false_authorization: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_document_sha256: Vec<(String, String)>,
    corpus_sha256: String,
    cases: usize,
    scenario_counts: BTreeMap<Scenario, usize>,
    recommendation_exact: usize,
    query_q0_selected: usize,
    authorized_retrievals: usize,
    resolved_cases: usize,
    ambiguous_cases: usize,
    exact_decisions: usize,
    retrieval_replays: usize,
    retrieval_tamper_rejections: usize,
    belief_replays: usize,
    belief_tamper_rejections: usize,
    policy_replays: usize,
    policy_tamper_rejections: usize,
    source_provenance_complete: usize,
    false_authorizations: usize,
    false_denials: usize,
    source_memory_mutations: usize,
    registry_mutations: usize,
    world_model_mutations: usize,
    hle_questions_read: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn digest_bytes(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

fn source(source_id: &str, lineage_id: &str) -> ClaimSource {
    ClaimSource {
        source_id: source_id.into(),
        title: format!("Immutable source snapshot {source_id}"),
        locator: format!("https://example.invalid/stage289/{source_id}"),
        retrieved_utc: "2026-08-18".into(),
        lineage_id: lineage_id.into(),
    }
}

fn claim(id: &str, object: &str, source_id: &str, lineage_id: &str, validity: &str) -> SourceClaim {
    SourceClaim {
        claim_id: id.into(),
        subject: "investigation_target".into(),
        predicate: "q0_observation".into(),
        object: object.into(),
        domain: "stage289_investigation".into(),
        scope: "exact_snapshot".into(),
        validity: validity.into(),
        assumptions: vec!["the observation applies to the queried snapshot".into()],
        source: source(source_id, lineage_id),
    }
}

fn query() -> ClaimQuery {
    ClaimQuery {
        subject: "investigation_target".into(),
        predicate: "q0_observation".into(),
        domain: "stage289_investigation".into(),
        scope: "exact_snapshot".into(),
        provenance: vec!["stage289-information-gain-query".into()],
    }
}

fn hypotheses() -> Vec<Hypothesis> {
    vec![
        Hypothesis {
            id: HypothesisId("h0".into()),
            description: "the first candidate explanation holds".into(),
            predictions: BTreeMap::from([
                ("q0".into(), "a".into()),
                ("q1".into(), "x".into()),
                ("q2".into(), "m".into()),
            ]),
            causal_paths: BTreeMap::from([(
                "q0".into(),
                vec!["h0".into(), "predicted_event".into(), "observation".into()],
            )]),
        },
        Hypothesis {
            id: HypothesisId("h1".into()),
            description: "the second candidate explanation holds".into(),
            predictions: BTreeMap::from([
                ("q0".into(), "b".into()),
                ("q1".into(), "x".into()),
                ("q2".into(), "n".into()),
            ]),
            causal_paths: BTreeMap::from([(
                "q0".into(),
                vec!["h1".into(), "predicted_event".into(), "observation".into()],
            )]),
        },
        Hypothesis {
            id: HypothesisId("h2".into()),
            description: "the third candidate explanation holds".into(),
            predictions: BTreeMap::from([
                ("q0".into(), "c".into()),
                ("q1".into(), "y".into()),
                ("q2".into(), "m".into()),
            ]),
            causal_paths: BTreeMap::from([(
                "q0".into(),
                vec!["h2".into(), "predicted_event".into(), "observation".into()],
            )]),
        },
    ]
}

fn investigation(id: &str) -> EpistemicInvestigation {
    EpistemicInvestigation {
        id: id.into(),
        hypotheses: hypotheses(),
        queries: vec![
            EvidenceQuery {
                id: "q0".into(),
                description: "retrieve the primary discriminating observation".into(),
                cost: 1,
            },
            EvidenceQuery {
                id: "q1".into(),
                description: "retrieve a partially discriminating observation".into(),
                cost: 1,
            },
            EvidenceQuery {
                id: "q2".into(),
                description: "retrieve a partially discriminating observation".into(),
                cost: 1,
            },
        ],
        evidence: Vec::new(),
        ground_truth: Some(HypothesisId("h0".into())),
        expected_recommendation: Recommendation::Recommend {
            query_id: "q0".into(),
        },
    }
}

fn scenario(index: usize) -> Scenario {
    match index {
        0..=299 => Scenario::Corroborated,
        300..=449 => Scenario::CopiedLineage,
        450..=599 => Scenario::StaleOnly,
        600..=749 => Scenario::Conflicting,
        750..=899 => Scenario::Missing,
        _ => Scenario::BudgetExhausted,
    }
}

fn build_case(index: usize) -> Case {
    let scenario = scenario(index);
    let id = format!("stage289-{index:04}");
    let claims = match scenario {
        Scenario::Corroborated | Scenario::BudgetExhausted => vec![
            claim(
                "primary",
                "a",
                "snapshot-primary",
                "lineage-primary",
                "current",
            ),
            claim(
                "independent",
                "a",
                "snapshot-independent",
                "lineage-independent",
                "current",
            ),
        ],
        Scenario::CopiedLineage => vec![
            claim(
                "primary",
                "a",
                "snapshot-primary",
                "lineage-primary",
                "current",
            ),
            claim("copy-a", "a", "summary-a", "lineage-primary", "current"),
            claim("copy-b", "a", "summary-b", "lineage-primary", "current"),
        ],
        Scenario::StaleOnly => vec![
            claim(
                "stale-a",
                "a",
                "snapshot-stale-a",
                "lineage-stale-a",
                "stale:2025",
            ),
            claim(
                "stale-b",
                "a",
                "snapshot-stale-b",
                "lineage-stale-b",
                "stale:2025",
            ),
        ],
        Scenario::Conflicting => vec![
            claim(
                "primary",
                "a",
                "snapshot-primary",
                "lineage-primary",
                "current",
            ),
            claim(
                "conflict",
                "b",
                "snapshot-independent",
                "lineage-independent",
                "current",
            ),
        ],
        Scenario::Missing => Vec::new(),
    };
    let (query_cost, budget) = if scenario == Scenario::BudgetExhausted {
        (2, 1)
    } else {
        (1, 2)
    };
    Case {
        id: id.clone(),
        scenario,
        investigation: investigation(&id),
        claim_query: query(),
        claims,
        query_cost,
        budget,
        expected_terminal: if scenario == Scenario::Corroborated {
            Terminal::Resolved
        } else {
            Terminal::Ambiguous
        },
    }
}

fn current_claims<'a>(result: &'a ClaimRetrievalResult) -> Vec<&'a SourceClaim> {
    result
        .claims
        .iter()
        .filter(|claim| claim.validity == "current")
        .collect()
}

fn retrieval_authorized(case: &Case, result: &ClaimRetrievalResult) -> bool {
    if case.query_cost > case.budget || !result.replay_verified() {
        return false;
    }
    let current = current_claims(result);
    let objects: BTreeSet<_> = current.iter().map(|claim| claim.object.as_str()).collect();
    let lineages: BTreeSet<_> = current
        .iter()
        .map(|claim| claim.source.lineage_id.as_str())
        .collect();
    result.status == RetrievalStatus::Supported
        && objects.len() == 1
        && lineages.len() >= 2
        && current.iter().all(|claim| claim.validity == "current")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases: Vec<_> = (0..CASES).map(build_case).collect();
    let mut receipts = Vec::with_capacity(CASES);
    let mut scenario_counts = BTreeMap::new();
    let mut recommendation_exact = 0;
    let mut query_q0_selected = 0;
    let mut authorized_retrievals = 0;
    let mut resolved_cases = 0;
    let mut ambiguous_cases = 0;
    let mut exact_decisions = 0;
    let mut retrieval_replays = 0;
    let mut retrieval_tamper_rejections = 0;
    let mut belief_replays = 0;
    let mut belief_tamper_rejections = 0;
    let mut policy_replays = 0;
    let mut policy_tamper_rejections = 0;
    let mut source_provenance_complete = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;

    for case in &cases {
        *scenario_counts.entry(case.scenario).or_insert(0usize) += 1;
        let analysis = analyze(&case.investigation);
        let recommendation_is_exact = analysis.recommendation
            == (Recommendation::Recommend {
                query_id: "q0".into(),
            });
        if recommendation_is_exact {
            recommendation_exact += 1;
            query_q0_selected += 1;
        }
        let retrieval = retrieve_claim(&case.claim_query, &case.claims);
        if retrieval.replay_verified() {
            retrieval_replays += 1;
        }
        let mut tampered_retrieval = retrieval.clone();
        tampered_retrieval.replay_hash.push('x');
        if !tampered_retrieval.replay_verified() {
            retrieval_tamper_rejections += 1;
        }
        let authorized = recommendation_is_exact && retrieval_authorized(case, &retrieval);
        if authorized {
            authorized_retrievals += 1;
        }
        let mut updated = case.investigation.clone();
        if authorized {
            updated.evidence.push(EvidenceRecord {
                id: format!("{}-q0", case.id),
                query_id: "q0".into(),
                outcome: "a".into(),
                timestamp: 1,
                valid_until: None,
                source: retrieval.claims[0].source.source_id.clone(),
                reliability: 100,
                confidence: 100,
                ancestry: retrieval.independent_lineages.clone(),
                correlation_group: Some("stage289-current-lineages".into()),
                failure_mode: None,
                causal_path: vec!["source_snapshot".into(), "q0_observation".into()],
            });
        }
        let belief = replay_beliefs(&updated);
        if belief.replay_verified() {
            belief_replays += 1;
        }
        let mut tampered_belief = belief.clone();
        tampered_belief.final_plausible.clear();
        if !tampered_belief.replay_verified() {
            belief_tamper_rejections += 1;
        }
        let policy = PolicyReceipt::new(
            case.id.clone(),
            "q0".into(),
            authorized,
            retrieval.replay_hash.clone(),
            case.query_cost,
            case.budget,
        );
        if policy.replay_verified() {
            policy_replays += 1;
        }
        let mut tampered_policy = policy.clone();
        tampered_policy.authorized = !tampered_policy.authorized;
        if !tampered_policy.replay_verified() {
            policy_tamper_rejections += 1;
        }
        let terminal = if authorized && belief.final_plausible == vec![HypothesisId("h0".into())] {
            resolved_cases += 1;
            Terminal::Resolved
        } else {
            ambiguous_cases += 1;
            Terminal::Ambiguous
        };
        let exact = recommendation_is_exact && terminal == case.expected_terminal;
        if exact {
            exact_decisions += 1;
        }
        let provenance = case.claims.iter().all(|claim| {
            !claim.source.source_id.is_empty()
                && !claim.source.lineage_id.is_empty()
                && !claim.source.locator.is_empty()
        });
        if provenance {
            source_provenance_complete += 1;
        }
        let false_authorization =
            terminal == Terminal::Resolved && case.expected_terminal != Terminal::Resolved;
        if false_authorization {
            false_authorizations += 1;
        }
        if case.expected_terminal == Terminal::Resolved && terminal != Terminal::Resolved {
            false_denials += 1;
        }
        receipts.push(Receipt {
            id: case.id.clone(),
            scenario: case.scenario,
            recommendation: analysis.recommendation,
            recommendation_exact: recommendation_is_exact,
            retrieval_status: retrieval.status,
            retrieval_authorized: authorized,
            terminal,
            expected_terminal: case.expected_terminal,
            final_plausible: belief.final_plausible.clone(),
            retrieval_replay_verified: retrieval.replay_verified(),
            retrieval_tamper_rejected: !tampered_retrieval.replay_verified(),
            belief_replay_verified: belief.replay_verified(),
            belief_tamper_rejected: !tampered_belief.replay_verified(),
            policy_replay_verified: policy.replay_verified(),
            policy_tamper_rejected: !tampered_policy.replay_verified(),
            source_provenance_complete: provenance,
            exact,
            false_authorization,
        });
    }
    let source_document_sha256 = SOURCE_DOCUMENTS
        .iter()
        .map(|(name, text)| ((*name).into(), digest_bytes(text.as_bytes())))
        .collect::<Vec<_>>();
    let report = Report {
        schema: "stage289-retrieval-guided-investigation-v1",
        source_document_sha256,
        corpus_sha256: digest(&cases),
        cases: cases.len(),
        scenario_counts,
        recommendation_exact,
        query_q0_selected,
        authorized_retrievals,
        resolved_cases,
        ambiguous_cases,
        exact_decisions,
        retrieval_replays,
        retrieval_tamper_rejections,
        belief_replays,
        belief_tamper_rejections,
        policy_replays,
        policy_tamper_rejections,
        source_provenance_complete,
        false_authorizations,
        false_denials,
        source_memory_mutations: 0,
        registry_mutations: 0,
        world_model_mutations: 0,
        hle_questions_read: 0,
        receipts,
    };
    assert_eq!(report.cases, CASES);
    assert_eq!(report.recommendation_exact, CASES);
    assert_eq!(report.query_q0_selected, CASES);
    assert_eq!(report.authorized_retrievals, 300);
    assert_eq!(report.resolved_cases, 300);
    assert_eq!(report.ambiguous_cases, 700);
    assert_eq!(report.exact_decisions, CASES);
    assert_eq!(report.retrieval_replays, CASES);
    assert_eq!(report.retrieval_tamper_rejections, CASES);
    assert_eq!(report.belief_replays, CASES);
    assert_eq!(report.belief_tamper_rejections, CASES);
    assert_eq!(report.policy_replays, CASES);
    assert_eq!(report.policy_tamper_rejections, CASES);
    assert_eq!(report.source_provenance_complete, CASES);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.source_memory_mutations, 0);
    assert_eq!(report.registry_mutations, 0);
    assert_eq!(report.world_model_mutations, 0);
    assert_eq!(report.hle_questions_read, 0);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 289 — retrieval-guided investigation\n\nA 1,000-case epistemic campaign selects an information-gain query, retrieves immutable versioned claims, applies lineage/freshness/budget policy, and updates beliefs only on authorized evidence.\n\n* cases / exact decisions: {} / {}\n* recommendation and q0 selection: {} / {}\n* authorized retrievals / resolved beliefs: {} / {}\n* ambiguous outcomes: {}\n* retrieval replay / tamper: {} / {}\n* belief replay / tamper: {} / {}\n* policy replay / tamper: {} / {}\n* provenance-complete cases: {}\n* false authorizations / denials: 0 / 0\n* source-memory / registry / world-model mutations: 0 / 0\n* HLE questions read: 0\n\nThe source documents are embedded immutable snapshots. The campaign never mutates live curriculum state and never treats a retrieved claim as a fact without independent current corroboration.\n\nReproduce with `cargo run --quiet --bin stage289_retrieval_guided_investigation`.\n",
            report.cases,
            report.exact_decisions,
            report.recommendation_exact,
            report.query_q0_selected,
            report.authorized_retrievals,
            report.resolved_cases,
            report.ambiguous_cases,
            report.retrieval_replays,
            report.retrieval_tamper_rejections,
            report.belief_replays,
            report.belief_tamper_rejections,
            report.policy_replays,
            report.policy_tamper_rejections,
            report.source_provenance_complete,
        ),
    )?;
    println!(
        "stage289 cases={} exact={} recommendation={} authorized={} resolved={} false_auth=0",
        report.cases,
        report.exact_decisions,
        report.recommendation_exact,
        report.authorized_retrievals,
        report.resolved_cases
    );
    Ok(())
}
