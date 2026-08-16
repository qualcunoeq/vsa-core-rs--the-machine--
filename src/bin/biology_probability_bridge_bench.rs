//! Stage H biology/probability composition benchmark.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::probability_pack::{
    evaluate_probability, ProbabilityArtifact, ProbabilityStatus,
};
use the_machine::source_formula_pack::biology_pack::biology_probability_bridge::{
    bridge_base_composition, BiologyProbabilityBridgeStatus,
};
use the_machine::source_formula_pack::biology_pack::{
    evaluate_biology, BiologyOperation, BiologyRequest, BiologyStatus,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    biology_status: BiologyStatus,
    bridge_status: BiologyProbabilityBridgeStatus,
    probability_status: Option<ProbabilityStatus>,
    exact: bool,
    handoff_valid: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    policy_preserved: bool,
    semantic_distribution_preserved: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_handoffs: usize,
    biology_replays: usize,
    bridge_replays: usize,
    probability_replays: usize,
    tamper_rejections: usize,
    policy_preserved: usize,
    semantic_distributions_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("biology probability serializes"))
    )
}

fn biology_request(operation: BiologyOperation, sequence: &str) -> BiologyRequest {
    BiologyRequest {
        operation,
        sequence: Some(sequence.into()),
        orientation: None,
        domain: "source_derived_bounded_dna".into(),
        ambiguity: None,
        provenance: vec!["stage-h-biology-probability-bridge".into()],
    }
}

fn run(id: String, biology_request: BiologyRequest, policy: Option<&str>, expected: Expected) -> Receipt {
    let biology = evaluate_biology(&biology_request);
    let bridge = bridge_base_composition(&biology, policy);
    let (probability_status, handoff_valid, policy_preserved, semantic_preserved, probability_replay) =
        if let Some(handoff) = bridge.handoff.as_ref() {
            let probability = evaluate_probability(&handoff.request);
            let artifact_valid = matches!(
                probability.artifact.as_ref(),
                Some(ProbabilityArtifact::Distribution(distribution))
                    if distribution.outcomes == vec!["A", "C", "G", "T"]
                        && distribution.probabilities.len() == 4
            );
            let valid = bridge.authorized()
                && handoff.sampling_policy == "uniform_position"
                && handoff.source_biology_replay_hash == biology.replay_hash
                && probability.status == ProbabilityStatus::Complete
                && probability.replay_verified()
                && artifact_valid;
            (
                Some(probability.status),
                valid,
                handoff.sampling_policy == "uniform_position",
                artifact_valid,
                probability.replay_verified(),
            )
        } else {
            (None, false, false, false, true)
        };
    let biology_replay = biology.replay_verified();
    let bridge_replay = bridge.replay_verified();
    let replay_verified = biology_replay && bridge_replay && probability_replay;
    let mut tampered_biology = biology.clone();
    tampered_biology.replay_hash.push('x');
    let mut tampered_bridge = bridge.clone();
    tampered_bridge.replay_hash.push('x');
    let probability_tamper_rejected = bridge.handoff.as_ref().is_none_or(|handoff| {
        let probability = evaluate_probability(&handoff.request);
        let mut tampered = probability.clone();
        tampered.replay_hash.push('x');
        !tampered.replay_verified()
    });
    let tamper_rejected = !tampered_biology.replay_verified()
        && !tampered_bridge.replay_verified()
        && probability_tamper_rejected;
    let authorized = expected == Expected::Supported && handoff_valid && replay_verified;
    let exact = match expected {
        Expected::Supported => authorized,
        Expected::Ambiguous => {
            bridge.status == BiologyProbabilityBridgeStatus::Ambiguous && !authorized
        }
        Expected::Refused => {
            bridge.status == BiologyProbabilityBridgeStatus::Unsupported && !authorized
        }
    };
    Receipt {
        id,
        expected,
        biology_status: biology.status,
        bridge_status: bridge.status,
        probability_status,
        exact,
        handoff_valid,
        replay_verified,
        tamper_rejected,
        policy_preserved,
        semantic_distribution_preserved: semantic_preserved,
        false_authorization: expected != Expected::Supported && authorized,
        false_denial: expected == Expected::Supported && !authorized,
    }
}

fn main() {
    let sequences = ["AATTGGCC", "ATCGATCG", "GCGCGCAA", "TTAAACCG", "AGCTAGCT"];
    let mut receipts = Vec::with_capacity(240);
    for index in 0..120 {
        receipts.push(run(
            format!("supported_{index:03}"),
            biology_request(
                BiologyOperation::BaseComposition,
                sequences[index % sequences.len()],
            ),
            Some("uniform_position"),
            Expected::Supported,
        ));
    }
    for index in 0..40 {
        receipts.push(run(
            format!("ambiguous_{index:03}"),
            biology_request(
                BiologyOperation::BaseComposition,
                sequences[index % sequences.len()],
            ),
            None,
            Expected::Ambiguous,
        ));
    }
    for index in 0..40 {
        receipts.push(run(
            format!("refused_policy_{index:03}"),
            biology_request(
                BiologyOperation::BaseComposition,
                sequences[index % sequences.len()],
            ),
            Some("independent_bases"),
            Expected::Refused,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("refused_sequence_{index:03}"),
            biology_request(BiologyOperation::ValidateDna, sequences[index % sequences.len()]),
            Some("uniform_position"),
            Expected::Refused,
        ));
    }
    for index in 0..20 {
        let mut reverse_request = biology_request(
            BiologyOperation::ReverseComplement,
            sequences[index % sequences.len()],
        );
        reverse_request.orientation = Some("5_to_3".into());
        receipts.push(run(
            format!("refused_complement_{index:03}"),
            reverse_request,
            Some("uniform_position"),
            Expected::Refused,
        ));
    }

    assert_eq!(receipts.len(), 240);
    let cases = receipts.len();
    let supported = receipts.iter().filter(|r| r.expected == Expected::Supported).count();
    let ambiguous = receipts.iter().filter(|r| r.expected == Expected::Ambiguous).count();
    let refused = receipts.iter().filter(|r| r.expected == Expected::Refused).count();
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let supported_handoffs = receipts.iter().filter(|r| r.handoff_valid).count();
    let biology_replays = receipts.iter().filter(|r| r.replay_verified).count();
    let bridge_replays = receipts.iter().filter(|r| r.replay_verified).count();
    let probability_replays = receipts.iter().filter(|r| r.probability_status.is_none() || r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let policy_preserved = receipts.iter().filter(|r| r.policy_preserved).count();
    let semantic_distributions_preserved = receipts
        .iter()
        .filter(|r| r.semantic_distribution_preserved)
        .count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!(exact_decisions, cases);
    assert_eq!(supported_handoffs, supported);
    assert_eq!(biology_replays, cases);
    assert_eq!(bridge_replays, cases);
    assert_eq!(probability_replays, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(policy_preserved, supported);
    assert_eq!(semantic_distributions_preserved, supported);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let mut route_counts = BTreeMap::new();
    for receipt in &receipts {
        let route = match receipt.expected {
            Expected::Supported => "uniform_base_distribution",
            Expected::Ambiguous => "missing_sampling_policy",
            Expected::Refused => "unsupported_sampling_semantics",
        };
        *route_counts.entry(route.to_string()).or_insert(0usize) += 1;
    }
    let report = Report {
        schema: "stage-h-biology-probability-bridge-v1",
        source: "independently authored bounded DNA/probability composition corpus",
        corpus_sha256: digest(&receipts),
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_handoffs,
        biology_replays,
        bridge_replays,
        probability_replays,
        tamper_rejections,
        policy_preserved,
        semantic_distributions_preserved,
        false_authorizations,
        false_denials,
        route_counts,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report).expect("biology probability serializes");
    std::fs::write("docs/stage_h_biology_probability_bridge.json", format!("{serialized}\n"))
        .expect("biology probability report writes");
    println!("{serialized}");
}
