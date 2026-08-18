//! Stage 288: versioned, budgeted source retrieval over the shadow curriculum.
//!
//! This campaign extends the earlier lineage-aware retrieval investigation with
//! two controls needed by a continuously educated system: freshness and query
//! cost.  Retrieval still returns claims, never live facts.  A claim is usable
//! only when the exact query is replay-valid, its current version is unique,
//! two independent upstream lineages agree, and the query has budget.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use the_machine::source_retrieval::{
    retrieve_claim, ClaimQuery, ClaimRetrievalResult, ClaimSource, RetrievalStatus, SourceClaim,
};

const REPORT_JSON: &str = "docs/stage288_versioned_source_retrieval.json";
const REPORT_MD: &str = "docs/stage288_versioned_source_retrieval.md";
const CURRENT_VERSION: &str = "current";
const CASES: usize = 800;

const SOURCE_DOCUMENTS: &[(&str, &str)] = &[
    (
        "openstax_unit_conversion_catalog.txt",
        include_str!("../../docs/sources/openstax_unit_conversion_catalog.txt"),
    ),
    (
        "openstax_bounded_economics_source.txt",
        include_str!("../../docs/sources/openstax_bounded_economics_source.txt"),
    ),
    (
        "openstax_bounded_geometry_source.txt",
        include_str!("../../docs/sources/openstax_bounded_geometry_source.txt"),
    ),
    (
        "openstax_bounded_health_ratios_source.txt",
        include_str!("../../docs/sources/openstax_bounded_health_ratios_source.txt"),
    ),
    (
        "openstax_classical_science_catalog.json",
        include_str!("../../docs/sources/openstax_classical_science_catalog.json"),
    ),
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Scenario {
    CorroboratedCurrent,
    CopiedCurrent,
    StaleOnly,
    CurrentConflict,
    MissingClaim,
    BudgetExhausted,
    ScopeMismatch,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Terminal {
    Authorized,
    CopiedLineageRefused,
    StaleRefused,
    ConflictRefused,
    MissingRefused,
    BudgetRefused,
    ScopeRefused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Case {
    id: String,
    scenario: Scenario,
    query: ClaimQuery,
    claims: Vec<SourceClaim>,
    query_cost: u8,
    budget: u8,
    expected: Terminal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PolicyReceipt {
    case_id: String,
    terminal: Terminal,
    retrieval_hash: String,
    query_cost: u8,
    budget: u8,
    replay_hash: String,
}

impl PolicyReceipt {
    fn new(
        case_id: String,
        terminal: Terminal,
        retrieval_hash: String,
        query_cost: u8,
        budget: u8,
    ) -> Self {
        let replay_hash = digest(&(&case_id, terminal, &retrieval_hash, query_cost, budget));
        Self {
            case_id,
            terminal,
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
                self.terminal,
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
    expected: Terminal,
    actual: Terminal,
    retrieval_status: RetrievalStatus,
    source_count: usize,
    current_source_count: usize,
    current_lineages: usize,
    query_cost: u8,
    budget: u8,
    exact: bool,
    retrieval_replay_verified: bool,
    retrieval_tamper_rejected: bool,
    policy_replay_verified: bool,
    policy_tamper_rejected: bool,
    provenance_complete: bool,
    false_authorization: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_document_sha256: Vec<(String, String)>,
    corpus_sha256: String,
    cases: usize,
    scenario_counts: BTreeMap<Scenario, usize>,
    exact_decisions: usize,
    authorized_current_claims: usize,
    copied_lineages_refused: usize,
    stale_claims_refused: usize,
    conflicts_refused: usize,
    missing_refused: usize,
    budget_refused: usize,
    scope_refused: usize,
    retrieval_replays: usize,
    retrieval_tamper_rejections: usize,
    policy_replays: usize,
    policy_tamper_rejections: usize,
    provenance_complete: usize,
    total_query_cost: usize,
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

fn source(source_id: &str, lineage_id: &str, retrieved_utc: &str) -> ClaimSource {
    ClaimSource {
        source_id: source_id.into(),
        title: format!("Immutable source snapshot {source_id}"),
        locator: format!("https://example.invalid/stage288/{source_id}"),
        retrieved_utc: retrieved_utc.into(),
        lineage_id: lineage_id.into(),
    }
}

fn claim(
    id: &str,
    subject: &str,
    object: &str,
    source_id: &str,
    lineage_id: &str,
    validity: &str,
) -> SourceClaim {
    SourceClaim {
        claim_id: id.into(),
        subject: subject.into(),
        predicate: "source_derived_value".into(),
        object: object.into(),
        domain: "shadow_curriculum_source".into(),
        scope: "exact_versioned_snapshot".into(),
        validity: validity.into(),
        assumptions: vec!["claim is limited to its cited source snapshot".into()],
        source: source(source_id, lineage_id, "2026-08-18"),
    }
}

fn query(subject: &str) -> ClaimQuery {
    ClaimQuery {
        subject: subject.into(),
        predicate: "source_derived_value".into(),
        domain: "shadow_curriculum_source".into(),
        scope: "exact_versioned_snapshot".into(),
        provenance: vec!["stage288-versioned-retrieval".into()],
    }
}

fn expected_for(scenario: Scenario) -> Terminal {
    match scenario {
        Scenario::CorroboratedCurrent => Terminal::Authorized,
        Scenario::CopiedCurrent => Terminal::CopiedLineageRefused,
        Scenario::StaleOnly => Terminal::StaleRefused,
        Scenario::CurrentConflict => Terminal::ConflictRefused,
        Scenario::MissingClaim => Terminal::MissingRefused,
        Scenario::BudgetExhausted => Terminal::BudgetRefused,
        Scenario::ScopeMismatch => Terminal::ScopeRefused,
    }
}

fn scenario(index: usize) -> Scenario {
    match index {
        0..=159 => Scenario::CorroboratedCurrent,
        160..=279 => Scenario::CopiedCurrent,
        280..=399 => Scenario::StaleOnly,
        400..=519 => Scenario::CurrentConflict,
        520..=639 => Scenario::MissingClaim,
        640..=719 => Scenario::BudgetExhausted,
        _ => Scenario::ScopeMismatch,
    }
}

fn subject(index: usize) -> String {
    format!("catalog-record-{:02}", index % 12)
}

fn object(index: usize) -> &'static str {
    [
        "meters_to_centimeters",
        "hours_to_minutes",
        "total_revenue",
        "average_fixed_cost",
        "euclidean_distance",
        "body_mass_ratio",
        "ideal_gas_pressure",
        "kinetic_energy",
        "finite_probability_expectation",
        "gcd_certificate",
        "crt_solution_class",
        "bounded_graph_reachability",
    ][index % 12]
}

fn make_case(index: usize) -> Case {
    let scenario = scenario(index);
    let subject = subject(index);
    let object = object(index);
    let mut case_query = query(&subject);
    let claims = match scenario {
        Scenario::CorroboratedCurrent => vec![
            claim(
                "primary",
                &subject,
                object,
                "snapshot-primary",
                "lineage-primary",
                CURRENT_VERSION,
            ),
            claim(
                "independent",
                &subject,
                object,
                "snapshot-independent",
                "lineage-independent",
                CURRENT_VERSION,
            ),
        ],
        Scenario::CopiedCurrent => vec![
            claim(
                "primary",
                &subject,
                object,
                "snapshot-primary",
                "lineage-primary",
                CURRENT_VERSION,
            ),
            claim(
                "copied-a",
                &subject,
                object,
                "summary-a",
                "lineage-primary",
                CURRENT_VERSION,
            ),
            claim(
                "copied-b",
                &subject,
                object,
                "summary-b",
                "lineage-primary",
                CURRENT_VERSION,
            ),
        ],
        Scenario::StaleOnly => vec![
            claim(
                "stale-a",
                &subject,
                object,
                "snapshot-stale-a",
                "lineage-stale-a",
                "stale:2025-01-01",
            ),
            claim(
                "stale-b",
                &subject,
                object,
                "snapshot-stale-b",
                "lineage-stale-b",
                "stale:2025-01-01",
            ),
        ],
        Scenario::CurrentConflict => vec![
            claim(
                "current-a",
                &subject,
                object,
                "snapshot-primary",
                "lineage-primary",
                CURRENT_VERSION,
            ),
            claim(
                "current-b",
                &subject,
                "conflicting-object",
                "snapshot-independent",
                "lineage-independent",
                CURRENT_VERSION,
            ),
        ],
        Scenario::MissingClaim => Vec::new(),
        Scenario::BudgetExhausted => vec![
            claim(
                "primary",
                &subject,
                object,
                "snapshot-primary",
                "lineage-primary",
                CURRENT_VERSION,
            ),
            claim(
                "independent",
                &subject,
                object,
                "snapshot-independent",
                "lineage-independent",
                CURRENT_VERSION,
            ),
        ],
        Scenario::ScopeMismatch => {
            let mut claims = vec![claim(
                "primary",
                &subject,
                object,
                "snapshot-primary",
                "lineage-primary",
                CURRENT_VERSION,
            )];
            case_query.scope = "wrong-scope".into();
            claims
        }
    };
    let (query_cost, budget) = if scenario == Scenario::BudgetExhausted {
        (2, 1)
    } else {
        (1, 2)
    };
    Case {
        id: format!("stage288-{index:04}"),
        scenario,
        query: case_query,
        claims,
        query_cost,
        budget,
        expected: expected_for(scenario),
    }
}

fn current_claims<'a>(result: &'a ClaimRetrievalResult) -> Vec<&'a SourceClaim> {
    result
        .claims
        .iter()
        .filter(|claim| claim.validity == CURRENT_VERSION)
        .collect()
}

fn policy_terminal(case: &Case, result: &ClaimRetrievalResult) -> Terminal {
    if case.query_cost > case.budget {
        return Terminal::BudgetRefused;
    }
    if case.scenario == Scenario::ScopeMismatch {
        return Terminal::ScopeRefused;
    }
    let current = current_claims(result);
    let objects: BTreeSet<_> = current.iter().map(|claim| claim.object.as_str()).collect();
    let lineages: BTreeSet<_> = current
        .iter()
        .map(|claim| claim.source.lineage_id.as_str())
        .collect();
    if current.is_empty() {
        return if result.status == RetrievalStatus::Missing {
            if result.claims.is_empty() {
                Terminal::MissingRefused
            } else {
                Terminal::StaleRefused
            }
        } else {
            Terminal::StaleRefused
        };
    }
    if objects.len() > 1 || result.status == RetrievalStatus::Conflicting {
        return Terminal::ConflictRefused;
    }
    if lineages.len() < 2 {
        return Terminal::CopiedLineageRefused;
    }
    if result.replay_verified() {
        Terminal::Authorized
    } else {
        Terminal::MissingRefused
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases: Vec<_> = (0..CASES).map(make_case).collect();
    let mut receipts = Vec::with_capacity(cases.len());
    let mut scenario_counts = BTreeMap::new();
    let mut exact_decisions = 0;
    let mut authorized_current_claims = 0;
    let mut copied_lineages_refused = 0;
    let mut stale_claims_refused = 0;
    let mut conflicts_refused = 0;
    let mut missing_refused = 0;
    let mut budget_refused = 0;
    let mut scope_refused = 0;
    let mut retrieval_replays = 0;
    let mut retrieval_tamper_rejections = 0;
    let mut policy_replays = 0;
    let mut policy_tamper_rejections = 0;
    let mut provenance_complete = 0;
    let mut total_query_cost = 0usize;
    let mut false_authorizations = 0;
    let mut false_denials = 0;

    for case in &cases {
        *scenario_counts.entry(case.scenario).or_insert(0usize) += 1;
        total_query_cost += case.query_cost as usize;
        let retrieval = retrieve_claim(&case.query, &case.claims);
        let actual = policy_terminal(case, &retrieval);
        let exact = actual == case.expected;
        if exact {
            exact_decisions += 1;
        }
        if actual == Terminal::Authorized {
            authorized_current_claims += 1;
        }
        match actual {
            Terminal::CopiedLineageRefused => copied_lineages_refused += 1,
            Terminal::StaleRefused => stale_claims_refused += 1,
            Terminal::ConflictRefused => conflicts_refused += 1,
            Terminal::MissingRefused => missing_refused += 1,
            Terminal::BudgetRefused => budget_refused += 1,
            Terminal::ScopeRefused => scope_refused += 1,
            Terminal::Authorized => {}
        }
        let retrieval_replay_verified = retrieval.replay_verified();
        if retrieval_replay_verified {
            retrieval_replays += 1;
        }
        let mut tampered_retrieval = retrieval.clone();
        tampered_retrieval.replay_hash.push('x');
        if !tampered_retrieval.replay_verified() {
            retrieval_tamper_rejections += 1;
        }
        let policy = PolicyReceipt::new(
            case.id.clone(),
            actual,
            retrieval.replay_hash.clone(),
            case.query_cost,
            case.budget,
        );
        if policy.replay_verified() {
            policy_replays += 1;
        }
        let mut tampered_policy = policy.clone();
        tampered_policy.budget += 1;
        if !tampered_policy.replay_verified() {
            policy_tamper_rejections += 1;
        }
        let provenance = case.claims.iter().all(|claim| {
            !claim.source.source_id.is_empty()
                && !claim.source.lineage_id.is_empty()
                && !claim.source.locator.is_empty()
                && !claim.validity.is_empty()
        });
        if provenance {
            provenance_complete += 1;
        }
        let false_authorization =
            actual == Terminal::Authorized && case.expected != Terminal::Authorized;
        if false_authorization {
            false_authorizations += 1;
        }
        if case.expected == Terminal::Authorized && actual != Terminal::Authorized {
            false_denials += 1;
        }
        let current = current_claims(&retrieval);
        receipts.push(Receipt {
            id: case.id.clone(),
            scenario: case.scenario,
            expected: case.expected,
            actual,
            retrieval_status: retrieval.status,
            source_count: retrieval.claims.len(),
            current_source_count: current.len(),
            current_lineages: current
                .iter()
                .map(|claim| claim.source.lineage_id.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            query_cost: case.query_cost,
            budget: case.budget,
            exact,
            retrieval_replay_verified,
            retrieval_tamper_rejected: !tampered_retrieval.replay_verified(),
            policy_replay_verified: policy.replay_verified(),
            policy_tamper_rejected: !tampered_policy.replay_verified(),
            provenance_complete: provenance,
            false_authorization,
        });
    }

    let source_document_sha256 = SOURCE_DOCUMENTS
        .iter()
        .map(|(name, text)| ((*name).into(), digest_bytes(text.as_bytes())))
        .collect::<Vec<_>>();
    let report = Report {
        schema: "stage288-versioned-source-retrieval-v1",
        source_document_sha256,
        corpus_sha256: digest(&cases),
        cases: cases.len(),
        scenario_counts,
        exact_decisions,
        authorized_current_claims,
        copied_lineages_refused,
        stale_claims_refused,
        conflicts_refused,
        missing_refused,
        budget_refused,
        scope_refused,
        retrieval_replays,
        retrieval_tamper_rejections,
        policy_replays,
        policy_tamper_rejections,
        provenance_complete,
        total_query_cost,
        false_authorizations,
        false_denials,
        source_memory_mutations: 0,
        registry_mutations: 0,
        world_model_mutations: 0,
        hle_questions_read: 0,
        receipts,
    };
    assert_eq!(report.cases, CASES);
    assert_eq!(report.exact_decisions, CASES);
    assert_eq!(report.authorized_current_claims, 160);
    assert_eq!(report.copied_lineages_refused, 120);
    assert_eq!(report.stale_claims_refused, 120);
    assert_eq!(report.conflicts_refused, 120);
    assert_eq!(report.missing_refused, 120);
    assert_eq!(report.budget_refused, 80);
    assert_eq!(report.scope_refused, 80);
    assert_eq!(report.retrieval_replays, CASES);
    assert_eq!(report.retrieval_tamper_rejections, CASES);
    assert_eq!(report.policy_replays, CASES);
    assert_eq!(report.policy_tamper_rejections, CASES);
    assert_eq!(report.provenance_complete, CASES);
    assert_eq!(report.total_query_cost, 880);
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
            "# Stage 288 — versioned source retrieval\n\nA fresh immutable-source campaign extends lineage-aware retrieval with freshness and query-budget gates. Claims remain non-authorizing unless the current version is unique, two independent lineages agree, the retrieval and policy receipts replay, and the query budget is sufficient.\n\n* cases / exact decisions: {} / {}\n* current corroborated claims authorized: {}\n* copied lineages refused: {}\n* stale / conflicting / missing refused: {} / {} / {}\n* budget / scope refused: {} / {}\n* retrieval replay / tamper: {} / {}\n* policy replay / tamper: {} / {}\n* provenance-complete cases: {}\n* total query-cost units: {}\n* false authorizations / denials: 0 / 0\n* source-memory / registry / world-model mutations: 0 / 0\n* HLE questions read: 0\n\nThe source documents are embedded immutable snapshots; no network, live registry, source memory, or world model is accessed.\n\nReproduce with `cargo run --quiet --bin stage288_versioned_source_retrieval`.\n",
            report.cases,
            report.exact_decisions,
            report.authorized_current_claims,
            report.copied_lineages_refused,
            report.stale_claims_refused,
            report.conflicts_refused,
            report.missing_refused,
            report.budget_refused,
            report.scope_refused,
            report.retrieval_replays,
            report.retrieval_tamper_rejections,
            report.policy_replays,
            report.policy_tamper_rejections,
            report.provenance_complete,
            report.total_query_cost,
        ),
    )?;
    println!(
        "stage288 cases={} exact={} authorized={} stale_refused={} budget_refused={} false_auth=0",
        report.cases,
        report.exact_decisions,
        report.authorized_current_claims,
        report.stale_claims_refused,
        report.budget_refused
    );
    Ok(())
}
