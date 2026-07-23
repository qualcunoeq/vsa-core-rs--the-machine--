use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
};
use the_machine::external_decomposition_benchmark::ExpectedOutcome;
use the_machine::gsm8k_quantity_candidate::formalize;
use the_machine::quantity_relation_integration::bridge_to_algebra;
use the_machine::raw_decomposition_benchmark::{decompose, realize, DecompositionDecision};
use the_machine::third_party_corpus_benchmark::ThirdPartyCorpus;

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
    family: String,
    expected_result: String,
}

fn main() {
    let config_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "data/third_party_gsm8k_quantity_candidate_v1.json".into());
    let config: CandidateRelease =
        serde_json::from_str(&fs::read_to_string(&config_path).expect("candidate release"))
            .expect("candidate JSON");
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
    assert!(
        promoted.keys().all(|id| base_ids.contains(id)),
        "promoted case missing from base release"
    );

    let mut structural = 0usize;
    let mut existing = 0usize;
    let mut quantity = 0usize;
    let mut quantity_replayed = 0usize;
    let mut ambiguous = 0usize;
    let mut unsupported = 0usize;
    let mut results_checked = 0usize;
    let mut result_correct = 0usize;
    let mut false_auth = 0usize;
    let mut false_denials = 0usize;
    let mut candidate_leakage = 0usize;
    let mut failures = BTreeMap::<String, usize>::new();

    for case in &base.cases {
        let expected_quantity = promoted.get(case.id.as_str()).copied();
        if expected_quantity.is_none() && formalize(&case.original_prompt).is_some() {
            candidate_leakage += 1;
        }
        let (actual_route, actual_result, actual_family, replayed) =
            if let Some(expected) = expected_quantity {
                match formalize(&case.original_prompt) {
                    Some(artifact) => {
                        let family = artifact.family.clone();
                        let replayed = artifact.replay_verified();
                        let result = bridge_to_algebra(&artifact).map(|receipt| receipt.result);
                        ("quantity_relation", result, Some(family), replayed)
                    }
                    None => ("quantity_relation", None, None, false),
                }
            } else {
                match decompose(&case.original_prompt) {
                    DecompositionDecision::Sketch(sketch) => {
                        let result = realize(&sketch).map(|(result, _)| result);
                        let realized = result.is_some();
                        ("existing", result, None, realized)
                    }
                    DecompositionDecision::Ambiguous => ("ambiguous", None, None, false),
                    DecompositionDecision::NoDecomposition => ("unsupported", None, None, false),
                }
            };

        let expected_route = if expected_quantity.is_some() {
            "quantity_relation"
        } else {
            match case.expected_outcome {
                ExpectedOutcome::Supported => "existing",
                ExpectedOutcome::Ambiguous => "ambiguous",
                ExpectedOutcome::Unsupported => "unsupported",
            }
        };
        let expected_result = expected_quantity
            .map(|case| case.expected_result.as_str())
            .or(case.expected_result.as_deref());
        let route_ok = actual_route == expected_route
            && expected_quantity
                .map(|case| actual_family == Some(case.family.clone()))
                .unwrap_or(true);
        let result_ok = expected_result
            .map(|expected| actual_result.as_deref() == Some(expected))
            .unwrap_or(true);
        let correct = route_ok && result_ok;
        structural += usize::from(correct);
        if actual_route == "existing" {
            existing += 1;
        }
        if actual_route == "quantity_relation" {
            quantity += 1;
            quantity_replayed += usize::from(replayed);
        }
        if actual_route == "ambiguous" {
            ambiguous += 1;
        }
        if actual_route == "unsupported" {
            unsupported += 1;
        }
        if expected_result.is_some() {
            results_checked += 1;
            result_correct += usize::from(result_ok);
        }
        let accepted = actual_result.is_some();
        let expected_supported =
            expected_route == "existing" || expected_route == "quantity_relation";
        false_auth += usize::from(accepted && !expected_supported);
        false_denials += usize::from(!accepted && expected_supported);
        if !correct {
            let label = if expected_supported && !accepted {
                "supported_not_realized"
            } else if accepted && !expected_supported {
                "false_authorization"
            } else if expected_quantity.is_some()
                && actual_family != Some(expected_quantity.unwrap().family.clone())
            {
                "quantity_family_mismatch"
            } else {
                "route_or_result_mismatch"
            };
            *failures.entry(label.into()).or_default() += 1;
        }
    }

    let hash = sha256_file(&config_path);
    println!(
        "gsm8k-quantity-candidate: release={} config_sha256={} cases={} structural={}/{} existing={} quantity_expected={} quantity_realized={} quantity_replayed={} ambiguous={} unsupported={} results={}/{} false_auth={} false_denials={} candidate_leakage={} failures={:?} deterministic=true",
        config.release_id, hash, base.cases.len(), structural, base.cases.len(), existing,
        config.promoted_cases.len(), quantity, quantity_replayed, ambiguous, unsupported,
        result_correct, results_checked, false_auth, false_denials, candidate_leakage, failures
    );
}

fn sha256_file(path: &str) -> String {
    use sha2::{Digest, Sha256};
    let bytes = fs::read(path).expect("candidate config bytes");
    format!("{:x}", Sha256::digest(bytes))
}
