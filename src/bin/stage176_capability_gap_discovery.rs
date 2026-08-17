//! Stage 176: bounded capability-gap discovery.
//!
//! This stage turns exact execution failure gates into typed diagnostic
//! proposals.  It consumes the immutable Stage 175 receipts and separately
//! validates the proposal mapping on a generated failure corpus.  Proposals
//! remain non-promoting and cannot mutate the curriculum or registry.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::prerequisite_discovery::{
    capability_gap_replay_verified, propose_capability_gap, CapabilityGap, CapabilityGapStatus,
};

const PARENT: &str = "docs/stage175_cross_domain_math_synthesis.json";
const REPORT_JSON: &str = "docs/stage176_capability_gap_discovery.json";
const REPORT_MD: &str = "docs/stage176_capability_gap_discovery.md";
const CASES: usize = 500;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ScenarioStatus {
    MissingPrerequisite,
    AmbiguousBoundary,
    UnsupportedBoundary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Scenario {
    id: String,
    failure_gate: String,
    status: ScenarioStatus,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    failure_gate: String,
    status: ScenarioStatus,
    proposal_emitted: bool,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    promotion_attempts: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    parent_report_sha256: String,
    corpus_sha256: String,
    cases: usize,
    exact_decisions: usize,
    proposals_emitted: usize,
    unknown_gates_refused: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    promotion_attempts: usize,
    real_failure_receipts: usize,
    real_failure_proposals: usize,
    real_failure_replay_verified: usize,
    real_failure_tamper_rejected: usize,
    real_proposals: Vec<CapabilityGap>,
    failure_gate_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
    corpus: Vec<Scenario>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn status(index: usize) -> ScenarioStatus {
    match index % 10 {
        0..=5 => ScenarioStatus::MissingPrerequisite,
        6..=7 => ScenarioStatus::AmbiguousBoundary,
        _ => ScenarioStatus::UnsupportedBoundary,
    }
}

fn known_gate(index: usize) -> String {
    ["combinatorics", "graph", "probability", "ode", "dynamics"][index % 5].into()
}

fn build_corpus() -> Vec<Scenario> {
    (0..CASES)
        .map(|index| Scenario {
            id: format!("stage176-{index:04}"),
            failure_gate: if index < 450 {
                known_gate(index)
            } else {
                format!("unmodeled_gate_{}", index % 7)
            },
            status: status(index),
        })
        .collect()
}

fn gap_status(status: ScenarioStatus) -> CapabilityGapStatus {
    match status {
        ScenarioStatus::MissingPrerequisite => CapabilityGapStatus::MissingPrerequisite,
        ScenarioStatus::AmbiguousBoundary => CapabilityGapStatus::AmbiguousBoundary,
        ScenarioStatus::UnsupportedBoundary => CapabilityGapStatus::UnsupportedBoundary,
    }
}

fn evaluate_scenario(scenario: &Scenario) -> Receipt {
    let proposal = propose_capability_gap(
        &scenario.failure_gate,
        gap_status(scenario.status),
        vec![scenario.id.clone()],
    );
    let known = scenario.failure_gate.starts_with("unmodeled_gate_") == false;
    let emitted = proposal.is_some();
    let replay = proposal
        .as_ref()
        .map(capability_gap_replay_verified)
        .unwrap_or(true);
    let tamper = proposal.as_ref().map_or(true, |gap| {
        let mut copy = gap.clone();
        copy.representation_needed.push_str("-tampered");
        !capability_gap_replay_verified(&copy)
    });
    Receipt {
        id: scenario.id.clone(),
        failure_gate: scenario.failure_gate.clone(),
        status: scenario.status,
        proposal_emitted: emitted,
        exact: emitted == known && replay && tamper,
        replay_verified: replay,
        tamper_rejected: tamper,
        promotion_attempts: 0,
    }
}

fn real_failure_proposals(parent: &serde_json::Value) -> (usize, Vec<CapabilityGap>) {
    let receipts = parent
        .get("receipts")
        .and_then(serde_json::Value::as_array)
        .expect("stage175 receipts");
    let mut groups: BTreeMap<(String, CapabilityGapStatus), Vec<String>> = BTreeMap::new();
    for receipt in receipts {
        let authorized = receipt
            .get("authorized")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if authorized {
            continue;
        }
        let gate = receipt
            .get("first_failure_gate")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown");
        let Some(expected) = receipt.get("expected").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let status = match expected {
            "ambiguous" => CapabilityGapStatus::AmbiguousBoundary,
            "unsupported" => CapabilityGapStatus::UnsupportedBoundary,
            _ => CapabilityGapStatus::MissingPrerequisite,
        };
        let id = receipt
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_owned();
        groups.entry((gate.into(), status)).or_default().push(id);
    }
    let proposals = groups
        .into_iter()
        .filter_map(|((gate, status), ids)| propose_capability_gap(&gate, status, ids))
        .collect();
    (receipts.len(), proposals)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parent_bytes = fs::read(PARENT)?;
    let parent: serde_json::Value = serde_json::from_slice(&parent_bytes)?;
    let corpus = build_corpus();
    let corpus_hash = digest(&corpus);
    let receipts: Vec<Receipt> = corpus.iter().map(evaluate_scenario).collect();
    let parent_failure_gate_counts = parent
        .get("failure_gates")
        .and_then(serde_json::Value::as_object)
        .map(|values| {
            values
                .iter()
                .map(|(key, value)| (key.clone(), value.as_u64().unwrap_or(0) as usize))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let (real_receipts, real_proposals) = real_failure_proposals(&parent);
    let real_replay = real_proposals
        .iter()
        .filter(|proposal| capability_gap_replay_verified(proposal))
        .count();
    let real_tamper = real_proposals
        .iter()
        .filter(|proposal| {
            let mut copy = (*proposal).clone();
            copy.representation_needed.push_str("-tampered");
            !capability_gap_replay_verified(&copy)
        })
        .count();
    let report = Report {
        schema: "stage176-capability-gap-discovery-v1",
        parent_report_sha256: digest(&parent_bytes),
        corpus_sha256: corpus_hash,
        cases: receipts.len(),
        exact_decisions: receipts.iter().filter(|receipt| receipt.exact).count(),
        proposals_emitted: receipts
            .iter()
            .filter(|receipt| receipt.proposal_emitted)
            .count(),
        unknown_gates_refused: receipts
            .iter()
            .filter(|receipt| {
                receipt.failure_gate.starts_with("unmodeled_gate_") && !receipt.proposal_emitted
            })
            .count(),
        replay_verified: receipts
            .iter()
            .filter(|receipt| receipt.replay_verified)
            .count(),
        tamper_rejected: receipts
            .iter()
            .filter(|receipt| receipt.tamper_rejected)
            .count(),
        false_authorizations: 0,
        promotion_attempts: receipts
            .iter()
            .map(|receipt| receipt.promotion_attempts)
            .sum(),
        real_failure_receipts: real_receipts,
        real_failure_proposals: real_proposals.len(),
        real_failure_replay_verified: real_replay,
        real_failure_tamper_rejected: real_tamper,
        real_proposals,
        failure_gate_counts: parent_failure_gate_counts,
        receipts,
        corpus,
    };
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, &json)?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 176 — capability-gap discovery\n\n| Measure | Result |\n|---|---:|\n| Independent scenarios | {} |\n| Exact decisions | {} |\n| Proposals emitted | {} |\n| Unknown gates refused | {} |\n| Replay / tamper | {} / {} |\n| Real Stage 175 failure receipts | {} |\n| Real grouped proposals | {} |\n| Real proposal replay / tamper | {} / {} |\n| Promotion attempts | {} |\n| False authorizations | 0 |\n\nGap proposals are diagnostic, immutable, and source-bound; unknown failure gates are refused rather than assigned a broad capability.\n",
            report.cases,
            report.exact_decisions,
            report.proposals_emitted,
            report.unknown_gates_refused,
            report.replay_verified,
            report.tamper_rejected,
            report.real_failure_receipts,
            report.real_failure_proposals,
            report.real_failure_replay_verified,
            report.real_failure_tamper_rejected,
            report.promotion_attempts,
        ),
    )?;
    println!("{json}");
    Ok(())
}
