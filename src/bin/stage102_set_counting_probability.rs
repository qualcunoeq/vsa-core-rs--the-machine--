//! Stage 102: explicit finite-set/count/probability composition.

use serde::Serialize;
use the_machine::probability_pack::{
    evaluate_probability, ProbabilityArtifact, ProbabilityOperation, ProbabilityRequest,
    ProbabilityStatus, Rational,
};
use the_machine::source_counting_pack::{
    evaluate as evaluate_count, replay_verified as count_replay, CountingArtifact,
    CountingOperation, CountingRequest, CountingStatus,
};
use the_machine::source_set_pack::{
    evaluate as evaluate_set, replay_verified as set_replay, SetArtifact, SetOperation, SetRequest,
    SetStatus,
};

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    supported: bool,
    authorized: bool,
    set_replay: bool,
    count_replay: bool,
    probability_replay: bool,
    tamper_rejected: bool,
    false_authorization: bool,
    false_denial: bool,
}
#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    cases: usize,
    supported: usize,
    refused: usize,
    authorized: usize,
    set_to_count_routes: usize,
    count_to_probability_routes: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<Receipt>,
}
fn main() {
    let mut receipts = Vec::new();
    for i in 0..240 {
        let supported = i % 2 == 0;
        let universe: std::collections::BTreeSet<String> =
            (0..(6 + i % 5)).map(|n| n.to_string()).collect();
        let left: std::collections::BTreeSet<String> = universe.iter().take(3).cloned().collect();
        let set_request = SetRequest {
            operation: SetOperation::Cardinality,
            universe: universe.clone(),
            left: left.clone(),
            right: Default::default(),
            ambiguity: (!supported)
                .then(|| "cardinality is not authorized for an ambiguous set target".into()),
            provenance: vec![format!("stage102:{i}")],
        };
        let set_result = evaluate_set(&set_request);
        let mut count_replay_ok = true;
        let mut probability_replay_ok = true;
        let mut authorized = false;
        let mut tamper = {
            let mut c = set_result.clone();
            c.replay_hash.push('x');
            !set_replay(&c)
        };
        if supported {
            if let Some(SetArtifact::Cardinality(cardinality)) = set_result.artifact.as_ref() {
                let count_request = CountingRequest {
                    operation: if i % 4 == 0 {
                        CountingOperation::Factorial
                    } else {
                        CountingOperation::Combination
                    },
                    n: Some(*cardinality as u64),
                    r: Some((1 + i % (*cardinality as usize).max(1)) as u64),
                    factors: Vec::new(),
                    ambiguity: None,
                    provenance: vec![format!("stage102:{i}"), "set-cardinality-bridge".into()],
                };
                let count_result = evaluate_count(&count_request);
                count_replay_ok = count_replay(&count_result);
                let mut count_tamper = count_result.clone();
                count_tamper.replay_hash.push('x');
                tamper &= !count_replay(&count_tamper);
                if let Some(CountingArtifact::ExactCount(count)) = count_result.artifact.as_ref() {
                    let size = *count as usize;
                    let outcomes: Vec<String> = (0..size).map(|n| format!("o{n}")).collect();
                    let probability_request = ProbabilityRequest {
                        operation: ProbabilityOperation::DistributionConstruction,
                        domain: "finite_exact_probability".into(),
                        outcomes: outcomes.clone(),
                        probabilities: outcomes
                            .iter()
                            .map(|_| Rational::new(1, size as i128).unwrap())
                            .collect(),
                        values: Vec::new(),
                        event_a: None,
                        event_b: None,
                        partition: Vec::new(),
                        conditional_values: Vec::new(),
                        prior_probability: None,
                        likelihood: None,
                        evidence: None,
                        ambiguity: None,
                        provenance: vec![
                            format!("stage102:{i}"),
                            "count-to-uniform-distribution-bridge".into(),
                        ],
                    };
                    let probability = evaluate_probability(&probability_request);
                    probability_replay_ok = probability.replay_verified();
                    let mut probability_tamper = probability.clone();
                    probability_tamper.replay_hash.push('x');
                    tamper &=
                        !probability.replay_verified() || !probability_tamper.replay_verified();
                    authorized = set_result.status == SetStatus::Complete
                        && count_result.status == CountingStatus::Complete
                        && probability.status == ProbabilityStatus::Complete
                        && matches!(
                            probability.artifact,
                            Some(ProbabilityArtifact::Distribution(_))
                        )
                        && count_replay_ok
                        && probability_replay_ok;
                }
            }
        }
        receipts.push(Receipt {
            id: format!("set_count_prob_{i:04}"),
            supported,
            authorized,
            set_replay: set_replay(&set_result),
            count_replay: count_replay_ok,
            probability_replay: probability_replay_ok,
            tamper_rejected: tamper,
            false_authorization: !supported && authorized,
            false_denial: supported && !authorized,
        });
    }
    assert_eq!(
        receipts
            .iter()
            .filter(|r| r.supported && r.authorized)
            .count(),
        120
    );
    assert_eq!(receipts.iter().filter(|r| r.false_authorization).count(), 0);
    assert_eq!(receipts.iter().filter(|r| r.false_denial).count(), 0);
    assert_eq!(
        receipts
            .iter()
            .filter(|r| !r.set_replay || !r.tamper_rejected)
            .count(),
        0
    );
    let report = Report {
        schema: "stage102-set-counting-probability-v1",
        cases: 240,
        supported: 120,
        refused: 120,
        authorized: receipts.iter().filter(|r| r.authorized).count(),
        set_to_count_routes: 120,
        count_to_probability_routes: 120,
        replay_verified: receipts
            .iter()
            .filter(|r| r.set_replay && r.count_replay && r.probability_replay)
            .count(),
        tamper_rejections: receipts.iter().filter(|r| r.tamper_rejected).count(),
        false_authorizations: 0,
        false_denials: 0,
        receipts,
    };
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
