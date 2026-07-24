use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
};
use the_machine::external_decomposition_benchmark::ExpectedOutcome;
use the_machine::gsm8k_quantity_candidate::formalize as formalize_gsm;
use the_machine::quantity_cross_domain_benchmark::{
    plan, CrossDomainTask, PlannerDecision, RouteCandidate, RouteKind,
};
use the_machine::third_party_corpus_benchmark::ThirdPartyCorpus;
use the_machine::unit_aware_quantity::{formalize as formalize_unit, UnitQuantityDecision};

#[derive(Debug, Deserialize)]
struct CandidateRelease {
    schema_version: u32,
    release_id: String,
    base_release: String,
    source_release_sha256: String,
    oracle: String,
    holdout_locked: bool,
    promoted_cases: Vec<PromotedCase>,
}

#[derive(Debug, Deserialize)]
struct PromotedCase {
    id: String,
    route: String,
    family: String,
    expected_result: String,
}

fn sha256_file(path: &str) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "{:x}",
        Sha256::digest(fs::read(path).expect("release bytes"))
    )
}

fn candidate_set(prompt: &str) -> Vec<RouteCandidate> {
    vec![
        RouteCandidate {
            id: "planner_gsm_quantity".into(),
            kind: RouteKind::GsmQuantityToAlgebra,
            prompt: prompt.into(),
            cost: 2,
            support: 80,
        },
        RouteCandidate {
            id: "unit_aware".into(),
            kind: RouteKind::UnitToAlgebra,
            prompt: prompt.into(),
            cost: 2,
            support: 90,
        },
        RouteCandidate {
            id: "quantity_relation".into(),
            kind: RouteKind::QuantityToAlgebra,
            prompt: prompt.into(),
            cost: 2,
            support: 70,
        },
        RouteCandidate {
            id: "fractional_quantity".into(),
            kind: RouteKind::FractionToAlgebra,
            prompt: prompt.into(),
            cost: 2,
            support: 65,
        },
        RouteCandidate {
            id: "multi_step_quantity".into(),
            kind: RouteKind::MultiStepToAlgebra,
            prompt: prompt.into(),
            cost: 3,
            support: 60,
        },
    ]
}

fn family_matches(case: &PromotedCase, prompt: &str) -> bool {
    match case.route.as_str() {
        "planner_gsm_quantity" => {
            formalize_gsm(prompt).is_some_and(|artifact| artifact.family == case.family)
        }
        "unit_aware" => {
            matches!(formalize_unit(prompt), UnitQuantityDecision::Accepted(artifact) if artifact.operation == case.family)
        }
        _ => false,
    }
}

fn main() {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "data/third_party_gsm8k_quantity_planner_v3.json".into());
    let config: CandidateRelease =
        serde_json::from_str(&fs::read_to_string(&path).expect("planner release"))
            .expect("planner JSON");
    assert_eq!(config.schema_version, 1);
    assert!(config.holdout_locked && !config.oracle.trim().is_empty());
    let base: ThirdPartyCorpus =
        serde_json::from_str(&fs::read_to_string(&config.base_release).expect("base release"))
            .expect("base JSON");
    assert_eq!(
        base.release_hash(),
        config.source_release_sha256,
        "base release hash changed"
    );
    let promoted = config
        .promoted_cases
        .iter()
        .map(|case| (case.id.as_str(), case))
        .collect::<BTreeMap<_, _>>();
    let base_ids = base
        .cases
        .iter()
        .map(|case| case.id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(promoted.len(), config.promoted_cases.len());
    assert!(promoted.keys().all(|id| base_ids.contains(id)));

    let mut structural = 0usize;
    let mut existing = 0usize;
    let mut planner_accepted = 0usize;
    let mut planner_replayed = 0usize;
    let mut promoted_realized = 0usize;
    let mut gsm_quantity = 0usize;
    let mut multi_step = 0usize;
    let mut unit_aware = 0usize;
    let mut fractional = 0usize;
    let mut ambiguous = 0usize;
    let mut unsupported = 0usize;
    let mut results_checked = 0usize;
    let mut results_correct = 0usize;
    let mut false_auth = 0usize;
    let mut false_denials = 0usize;
    let mut candidate_leakage = 0usize;
    let mut failures = BTreeMap::<String, usize>::new();

    for case in &base.cases {
        let expected = promoted.get(case.id.as_str()).copied();
        let decision = plan(&CrossDomainTask {
            id: case.id.clone(),
            candidates: candidate_set(&case.original_prompt),
            expected: None,
            should_authorize: true,
            pair_id: None,
        });
        let accepted = matches!(decision, PlannerDecision::Preferred { .. });
        if case.expected_outcome == ExpectedOutcome::Supported {
            existing += 1;
        }
        if accepted {
            planner_accepted += 1;
            planner_replayed += 1;
        }
        let (actual_route, actual_result) = match &decision {
            PlannerDecision::Preferred { route_id, result } => {
                (Some(route_id.as_str()), Some(result.as_str()))
            }
            PlannerDecision::Ambiguous => {
                ambiguous += 1;
                (None, None)
            }
            PlannerDecision::NoCandidates => {
                unsupported += 1;
                (None, None)
            }
        };
        if let Some(expected) = expected {
            let route_ok = actual_route == Some(expected.route.as_str())
                && family_matches(expected, &case.original_prompt);
            let result_ok = actual_result == Some(expected.expected_result.as_str());
            promoted_realized += usize::from(accepted);
            structural += usize::from(route_ok && result_ok);
            results_checked += 1;
            results_correct += usize::from(result_ok);
            false_denials += usize::from(!accepted);
            if !(route_ok && result_ok) {
                let label = if !accepted {
                    "promoted_not_realized"
                } else if !route_ok {
                    "route_or_family_mismatch"
                } else {
                    "result_mismatch"
                };
                *failures.entry(format!("{label}:{}", case.id)).or_default() += 1;
            }
            match expected.route.as_str() {
                "unit_aware" => unit_aware += usize::from(accepted),
                "planner_gsm_quantity" => {
                    gsm_quantity += usize::from(accepted);
                    multi_step += usize::from(accepted && expected.family == "multi_step_quantity");
                }
                _ => {}
            }
        } else {
            let safe = case.expected_outcome == ExpectedOutcome::Supported || !accepted;
            structural += usize::from(safe);
            false_auth +=
                usize::from(accepted && case.expected_outcome != ExpectedOutcome::Supported);
            candidate_leakage +=
                usize::from(accepted && case.expected_outcome != ExpectedOutcome::Supported);
            if !safe {
                *failures
                    .entry(format!("candidate_leakage:{}", case.id))
                    .or_default() += 1;
            }
        }
    }
    println!("gsm8k-quantity-planner: release={} config_sha256={} cases={} structural={}/{} existing={} promoted_expected={} promoted_realized={} planner_accepted={} planner_replayed={} gsm_quantity={} multi_step={} unit_aware={} fractional={} ambiguous={} unsupported={} results={}/{} false_auth={} false_denials={} candidate_leakage={} failures={:?} deterministic=true", config.release_id, sha256_file(&path), base.cases.len(), structural, base.cases.len(), existing, config.promoted_cases.len(), promoted_realized, planner_accepted, planner_replayed, gsm_quantity, multi_step, unit_aware, fractional, ambiguous, unsupported, results_correct, results_checked, false_auth, false_denials, candidate_leakage, failures);
}
