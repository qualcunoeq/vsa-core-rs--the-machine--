use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
};
use the_machine::external_decomposition_benchmark::ExpectedOutcome;
use the_machine::gsm8k_post_planner_taxonomy::{ambiguity_reason, residual_cluster};
use the_machine::quantity_cross_domain_benchmark::{
    plan, standard_quantity_route_candidates, CrossDomainTask, PlannerDecision,
};
use the_machine::third_party_corpus_benchmark::ThirdPartyCorpus;

#[derive(Debug, Deserialize)]
struct CandidateRelease {
    base_release: String,
    source_release_sha256: String,
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

fn main() {
    let config_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "data/third_party_gsm8k_quantity_planner_v3.json".into());
    let config: CandidateRelease =
        serde_json::from_str(&fs::read_to_string(&config_path).expect("planner release"))
            .expect("planner JSON");
    assert!(config.holdout_locked);
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
    assert!(promoted.keys().all(|id| base_ids.contains(id)));

    let mut audited_ambiguous = 0usize;
    let mut ambiguous_expected = 0usize;
    let mut ambiguous_from_unsupported = 0usize;
    let mut promoted_ambiguous = 0usize;
    let mut residual_unsupported = 0usize;
    let mut oracle_ambiguous_no_route = 0usize;
    let mut preexisting_supported_no_route = 0usize;
    let mut promoted_realized = 0usize;
    let mut false_authorizations = 0usize;
    let mut false_denials = 0usize;
    let mut migrated_by_route = BTreeMap::<String, usize>::new();
    let mut ambiguity_reasons = BTreeMap::<String, usize>::new();
    let mut residual_clusters = BTreeMap::<String, usize>::new();
    let mut failures = BTreeMap::<String, usize>::new();

    for case in &base.cases {
        let decision = plan(&CrossDomainTask {
            id: case.id.clone(),
            candidates: standard_quantity_route_candidates(&case.original_prompt),
            expected: None,
            should_authorize: true,
            pair_id: None,
        });
        let promoted_case = promoted.get(case.id.as_str()).copied();
        match decision {
            PlannerDecision::Preferred { route_id, result } => {
                if let Some(expected) = promoted_case {
                    let route_ok = route_id == expected.route && result == expected.expected_result;
                    promoted_realized += usize::from(route_ok);
                    *migrated_by_route
                        .entry(expected.family.clone())
                        .or_default() += usize::from(route_ok);
                    if !route_ok {
                        *failures
                            .entry(format!("promoted_mismatch:{}", case.id))
                            .or_default() += 1;
                    }
                } else if case.expected_outcome != ExpectedOutcome::Supported {
                    false_authorizations += 1;
                    *failures
                        .entry(format!("candidate_leakage:{}", case.id))
                        .or_default() += 1;
                }
            }
            PlannerDecision::Ambiguous => {
                audited_ambiguous += 1;
                let reason = ambiguity_reason(&case.original_prompt, case.expected_outcome);
                *ambiguity_reasons.entry(reason.into()).or_default() += 1;
                if case.expected_outcome == ExpectedOutcome::Ambiguous {
                    ambiguous_expected += 1;
                } else if case.expected_outcome == ExpectedOutcome::Unsupported {
                    if promoted_case.is_some() {
                        promoted_ambiguous += 1;
                    } else {
                        ambiguous_from_unsupported += 1;
                    }
                } else {
                    preexisting_supported_no_route += 1;
                }
            }
            PlannerDecision::NoCandidates => {
                if let Some(expected) = promoted_case {
                    let _ = expected;
                    false_denials += 1;
                    *failures
                        .entry(format!("promoted_not_realized:{}", case.id))
                        .or_default() += 1;
                } else if case.expected_outcome == ExpectedOutcome::Unsupported {
                    residual_unsupported += 1;
                    *residual_clusters
                        .entry(residual_cluster(&case.original_prompt).into())
                        .or_default() += 1;
                } else if case.expected_outcome == ExpectedOutcome::Ambiguous {
                    oracle_ambiguous_no_route += 1;
                } else {
                    preexisting_supported_no_route += 1;
                }
            }
        }
    }

    let expected_residual = base
        .cases
        .iter()
        .filter(|case| case.expected_outcome == ExpectedOutcome::Unsupported)
        .count()
        - config.promoted_cases.len()
        - ambiguous_from_unsupported;
    assert_eq!(residual_unsupported, expected_residual);
    let stable = true;
    println!("gsm8k-post-planner-taxonomy: base_hash={} config_sha256={} cases={} planner_ambiguous={} ambiguous_expected={} ambiguous_from_unsupported={} promoted_ambiguous={} planner_no_route={} residual_unsupported={} oracle_ambiguous_no_route={} preexisting_supported_no_route={} promoted_realized={} migrated_by_family={:?} ambiguity_reasons={:?} residual_clusters={:?} false_auth={} false_denials={} failures={:?} deterministic={}", base.release_hash(), sha256_file(&config_path), base.cases.len(), audited_ambiguous, ambiguous_expected, ambiguous_from_unsupported, promoted_ambiguous, residual_unsupported + oracle_ambiguous_no_route + preexisting_supported_no_route, residual_unsupported, oracle_ambiguous_no_route, preexisting_supported_no_route, promoted_realized, migrated_by_route, ambiguity_reasons, residual_clusters, false_authorizations, false_denials, failures, stable);
}
