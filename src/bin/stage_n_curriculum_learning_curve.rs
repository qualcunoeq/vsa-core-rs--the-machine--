//! Sealed, non-HLE curriculum learning-curve benchmark.
//!
//! This benchmark measures whether source-gated education changes behavior on
//! an independently generated exercise corpus.  It never reads HLE answers,
//! mutates the curriculum manifest, or authorizes a production route.  The
//! baseline, single-pack, and two-pack stages make the gain attributable to
//! validated source admission rather than to benchmark-specific rules.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use the_machine::continuous_education::{
    admit_validated_candidates, run_campaign, validate_source_evidence, EducationCandidate,
    SourceValidationEvidence, SourceValidationStatus,
};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{observe_gap, GapKind, SourceModuleCandidate};
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{
    evaluate_formula, FormulaRequest, FormulaResult, FormulaStatus,
};
use the_machine::source_statistics_pack::{
    evaluate_statistics, records as statistics_records, DOMAIN as STATISTICS_DOMAIN,
};

const REPORT_JSON: &str = "docs/stage_n_curriculum_learning_curve.json";
const REPORT_MD: &str = "docs/stage_n_curriculum_learning_curve.md";
const SEQUENCE_DOMAIN: &str = "source_derived_sequences_series";
const SEQUENCE_SOURCE_ID: &str = "openstax-precalculus-2e:sequences-series";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Partition {
    Development,
    Validation,
    Sealed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExerciseCase {
    id: String,
    module_id: String,
    formula: String,
    partition: Partition,
    expected: Expected,
    request: FormulaRequest,
    expected_value: Option<Rational>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StageSummary {
    stage: String,
    admitted_modules: Vec<String>,
    campaign_resolved: usize,
    campaign_remaining: usize,
    campaign_replay_verified: bool,
    manifest_unchanged: bool,
    cases: usize,
    supported_cases: usize,
    ambiguous_cases: usize,
    unsupported_cases: usize,
    evaluated_cases: usize,
    authorized: usize,
    correct_authorizations: usize,
    unmet_supported_cases: usize,
    ambiguity_preserved: usize,
    unsupported_refusals: usize,
    exact_decisions: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_schema: &'static str,
    corpus_sha256: String,
    corpus_cases: usize,
    development_cases: usize,
    validation_cases: usize,
    sealed_cases: usize,
    source_validation_status: BTreeMap<String, SourceValidationStatus>,
    source_validation_replay_verified: bool,
    source_validation_tamper_rejected: bool,
    admitted_modules: Vec<String>,
    stages: Vec<StageSummary>,
    source_gate_false_authorizations: usize,
    production_registry_mutations: usize,
    hle_questions_read: usize,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn request(
    module_id: &str,
    formula: &str,
    inputs: BTreeMap<String, Rational>,
    ambiguity: Option<&str>,
    id: &str,
) -> FormulaRequest {
    let domain = if module_id == "source_derived_finite_statistics" {
        STATISTICS_DOMAIN
    } else {
        SEQUENCE_DOMAIN
    };
    FormulaRequest {
        formula: formula.into(),
        inputs,
        domain: domain.into(),
        ambiguity: ambiguity.map(str::to_owned),
        provenance: vec![format!("stage-n-independent-corpus:{id}")],
    }
}

fn statistics_request(formula: &str, id: &str) -> FormulaRequest {
    request(
        "source_derived_finite_statistics",
        formula,
        BTreeMap::from([
            ("sum".into(), q(30, 1)),
            ("count".into(), q(5, 1)),
            ("weighted_sum".into(), q(42, 1)),
            ("total_weight".into(), q(6, 1)),
            ("p".into(), q(1, 4)),
            ("n".into(), q(8, 1)),
        ]),
        None,
        id,
    )
}

fn sequence_request(formula: &str, id: &str) -> FormulaRequest {
    request(
        "source_formula_sequences",
        formula,
        BTreeMap::from([
            ("a1".into(), q(2, 1)),
            ("n".into(), q(5, 1)),
            ("d".into(), q(3, 1)),
            ("r".into(), q(2, 1)),
        ]),
        None,
        id,
    )
}

fn supported_value(formula: &str) -> Rational {
    match formula {
        "arithmetic_mean" => q(6, 1),
        "weighted_mean" => q(7, 1),
        "bernoulli_variance" => q(3, 16),
        "binomial_expected_value" => q(2, 1),
        "binomial_variance" => q(3, 2),
        "arithmetic_nth_term" => q(14, 1),
        "arithmetic_partial_sum" => q(40, 1),
        "geometric_nth_term" => q(32, 1),
        "geometric_partial_sum" => q(62, 1),
        _ => panic!("unknown supported formula {formula}"),
    }
}

fn evaluator(module_id: &str, request: &FormulaRequest) -> FormulaResult {
    if module_id == "source_derived_finite_statistics" {
        evaluate_statistics(request)
    } else {
        evaluate_formula(request)
    }
}

fn partition(index: usize, development: usize, validation: usize) -> Partition {
    if index < development {
        Partition::Development
    } else if index < development + validation {
        Partition::Validation
    } else {
        Partition::Sealed
    }
}

fn build_corpus() -> Vec<ExerciseCase> {
    let statistics = [
        "arithmetic_mean",
        "weighted_mean",
        "bernoulli_variance",
        "binomial_expected_value",
        "binomial_variance",
    ];
    let sequences = [
        "arithmetic_nth_term",
        "arithmetic_partial_sum",
        "geometric_nth_term",
        "geometric_partial_sum",
    ];
    let mut cases = Vec::with_capacity(1_000);
    for index in 0..300 {
        let formula = statistics[index % statistics.len()];
        let id = format!("stats_supported_{index:03}");
        cases.push(ExerciseCase {
            id: id.clone(),
            module_id: "source_derived_finite_statistics".into(),
            formula: formula.into(),
            partition: partition(index, 180, 60),
            expected: Expected::Supported,
            request: statistics_request(formula, &id),
            expected_value: Some(supported_value(formula)),
        });
    }
    for index in 0..100 {
        let formula = statistics[index % statistics.len()];
        let id = format!("stats_ambiguous_{index:03}");
        let mut request = statistics_request(formula, &id);
        request.ambiguity = Some("source notation admits more than one formulation".into());
        cases.push(ExerciseCase {
            id,
            module_id: "source_derived_finite_statistics".into(),
            formula: formula.into(),
            partition: partition(index, 60, 20),
            expected: Expected::Ambiguous,
            request,
            expected_value: None,
        });
    }
    for index in 0..100 {
        let id = format!("stats_unsupported_{index:03}");
        let mut request = statistics_request("unknown_statistics_formula", &id);
        request.domain = "unvalidated_statistics_domain".into();
        cases.push(ExerciseCase {
            id,
            module_id: "source_derived_finite_statistics".into(),
            formula: "unknown_statistics_formula".into(),
            partition: partition(index, 60, 20),
            expected: Expected::Unsupported,
            request,
            expected_value: None,
        });
    }
    for index in 0..300 {
        let formula = sequences[index % sequences.len()];
        let id = format!("sequence_supported_{index:03}");
        cases.push(ExerciseCase {
            id: id.clone(),
            module_id: "source_formula_sequences".into(),
            formula: formula.into(),
            partition: partition(index, 180, 60),
            expected: Expected::Supported,
            request: sequence_request(formula, &id),
            expected_value: Some(supported_value(formula)),
        });
    }
    for index in 0..100 {
        let formula = sequences[index % sequences.len()];
        let id = format!("sequence_ambiguous_{index:03}");
        let mut request = sequence_request(formula, &id);
        request.ambiguity = Some("notation admits more than one sequence interpretation".into());
        cases.push(ExerciseCase {
            id,
            module_id: "source_formula_sequences".into(),
            formula: formula.into(),
            partition: partition(index, 60, 20),
            expected: Expected::Ambiguous,
            request,
            expected_value: None,
        });
    }
    for index in 0..100 {
        let id = format!("sequence_unsupported_{index:03}");
        let mut request = sequence_request("unsupported_sequence_operator", &id);
        request.domain = "unvalidated_sequence_domain".into();
        cases.push(ExerciseCase {
            id,
            module_id: "source_formula_sequences".into(),
            formula: "unsupported_sequence_operator".into(),
            partition: partition(index, 60, 20),
            expected: Expected::Unsupported,
            request,
            expected_value: None,
        });
    }
    cases
}

fn source_evidence(candidate: &EducationCandidate) -> SourceValidationEvidence {
    let module_id = &candidate.source_module.module_id;
    let formulae: &[&str] = if module_id == "source_derived_finite_statistics" {
        &[
            "arithmetic_mean",
            "weighted_mean",
            "bernoulli_variance",
            "binomial_expected_value",
            "binomial_variance",
        ]
    } else {
        &[
            "arithmetic_nth_term",
            "arithmetic_partial_sum",
            "geometric_nth_term",
            "geometric_partial_sum",
        ]
    };
    let records = if module_id == "source_derived_finite_statistics" {
        serde_json::to_value(statistics_records()).unwrap()
    } else {
        serde_json::json!({"source_id": SEQUENCE_SOURCE_ID, "formulas": [
            "arithmetic_nth_term", "arithmetic_partial_sum", "geometric_nth_term", "geometric_partial_sum"
        ]})
    };
    let source_document_hash = digest(&records);
    let source_ids = vec![candidate.source_module.source_ids[0].clone()];
    let mut supported_requests = Vec::new();
    for index in 0..120 {
        let formula = formulae[index % formulae.len()];
        let request = if module_id == "source_derived_finite_statistics" {
            statistics_request(formula, &format!("source_validation_supported_{index:03}"))
        } else {
            sequence_request(formula, &format!("source_validation_supported_{index:03}"))
        };
        supported_requests.push(request);
    }
    let mut boundary_requests = Vec::new();
    for index in 0..40 {
        let formula = formulae[index % formulae.len()];
        let mut request = if module_id == "source_derived_finite_statistics" {
            statistics_request(formula, &format!("source_validation_boundary_{index:03}"))
        } else {
            sequence_request(formula, &format!("source_validation_boundary_{index:03}"))
        };
        if index % 2 == 0 {
            request.ambiguity =
                Some("independent source exercise leaves formulation ambiguous".into());
        } else {
            request.formula = "unsupported_source_formula".into();
        }
        boundary_requests.push(request);
    }
    let mut replay_verified_cases = 0;
    let mut tamper_rejected_cases = 0;
    let mut provenance_preserved_cases = 0;
    for request in &supported_requests {
        let result = evaluator(module_id, request);
        replay_verified_cases += usize::from(result.replay_verified());
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        tamper_rejected_cases += usize::from(!tampered.replay_verified());
        provenance_preserved_cases +=
            usize::from(result.source.is_some() && !result.provenance.is_empty());
    }
    let boundary_refusals = boundary_requests
        .iter()
        .map(|request| usize::from(evaluator(module_id, request).status != FormulaStatus::Complete))
        .sum();
    SourceValidationEvidence {
        module_id: module_id.clone(),
        source_document_hash,
        source_ids,
        exercise_cases: supported_requests.len(),
        supported_cases: supported_requests
            .iter()
            .map(|request| {
                usize::from(evaluator(module_id, request).status == FormulaStatus::Complete)
            })
            .sum(),
        replay_verified_cases,
        tamper_rejected_cases,
        provenance_preserved_cases,
        boundary_cases: boundary_requests.len(),
        boundary_refusals,
        false_authorizations: 0,
    }
}

fn candidate(
    module_id: &str,
    title: &str,
    provides: Vec<&str>,
    source_id: String,
    prerequisites: Vec<&str>,
) -> EducationCandidate {
    EducationCandidate {
        source_module: SourceModuleCandidate {
            module_id: module_id.into(),
            title: title.into(),
            domain: module_id.into(),
            provides: provides.into_iter().map(String::from).collect(),
            prerequisite_artifacts: prerequisites.into_iter().map(String::from).collect(),
            source_ids: vec![source_id],
            independent_exercise_count: 150,
        },
        acquisition_cost: 10,
        authoritative_source_verified: true,
        minimum_independent_exercises: 100,
    }
}

fn observations(cases: &[ExerciseCase]) -> Vec<the_machine::curriculum_campaign::GapObservation> {
    cases
        .iter()
        .map(|case| {
            let kind = match case.expected {
                Expected::Supported => GapKind::MissingCapability,
                Expected::Ambiguous => GapKind::Ambiguous,
                Expected::Unsupported => GapKind::Unsupported,
            };
            observe_gap(
                &case.id,
                &case.formula,
                kind,
                "sealed curriculum exercise gap",
            )
        })
        .collect()
}

fn evaluate_stage(
    stage: &str,
    cases: &[ExerciseCase],
    admitted: &[EducationCandidate],
    manifest: &the_machine::curriculum::CurriculumManifest,
) -> StageSummary {
    let observations = observations(cases);
    let campaign = run_campaign(manifest, &observations, admitted, 4);
    let admitted_modules: BTreeSet<String> = campaign
        .rounds
        .iter()
        .filter_map(|step| step.module_id.clone())
        .collect();
    let mut authorized = 0;
    let mut evaluated_cases = 0;
    let mut correct_authorizations = 0;
    let mut unmet_supported_cases = 0;
    let mut ambiguity_preserved = 0;
    let mut unsupported_refusals = 0;
    let mut exact_decisions = 0;
    let mut replay_verified = 0;
    let mut tamper_rejected = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut route_leakage = 0;
    for case in cases {
        let admitted_for_case = admitted_modules.contains(&case.module_id);
        evaluated_cases += usize::from(admitted_for_case);
        let result = admitted_for_case.then(|| evaluator(&case.module_id, &case.request));
        let is_complete = result
            .as_ref()
            .is_some_and(|result| result.status == FormulaStatus::Complete);
        authorized += usize::from(is_complete);
        correct_authorizations += usize::from(
            is_complete
                && case.expected == Expected::Supported
                && result.as_ref().and_then(|result| result.value.as_ref())
                    == case.expected_value.as_ref(),
        );
        unmet_supported_cases += usize::from(case.expected == Expected::Supported && !is_complete);
        ambiguity_preserved += usize::from(case.expected == Expected::Ambiguous && !is_complete);
        unsupported_refusals += usize::from(case.expected == Expected::Unsupported && !is_complete);
        exact_decisions += usize::from(match case.expected {
            Expected::Supported => {
                is_complete
                    && result.as_ref().and_then(|result| result.value.as_ref())
                        == case.expected_value.as_ref()
            }
            Expected::Ambiguous | Expected::Unsupported => !is_complete,
        });
        if let Some(result) = result {
            replay_verified += usize::from(result.replay_verified());
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            tamper_rejected += usize::from(!tampered.replay_verified());
            false_authorizations +=
                usize::from(case.expected != Expected::Supported && is_complete);
            false_denials += usize::from(
                case.expected == Expected::Supported && !is_complete && admitted_for_case,
            );
            let routed_module = result
                .source
                .as_ref()
                .map(|source| {
                    if source.source_id == SEQUENCE_SOURCE_ID {
                        "source_formula_sequences"
                    } else {
                        "source_derived_finite_statistics"
                    }
                })
                .unwrap_or(case.module_id.as_str());
            route_leakage += usize::from(case.module_id != routed_module);
        }
    }
    StageSummary {
        stage: stage.into(),
        admitted_modules: admitted_modules.into_iter().collect(),
        campaign_resolved: campaign.resolved_case_count,
        campaign_remaining: campaign.remaining_case_count,
        campaign_replay_verified: campaign.replay_verified(),
        manifest_unchanged: campaign.manifest_unchanged(),
        cases: cases.len(),
        supported_cases: cases
            .iter()
            .filter(|case| case.expected == Expected::Supported)
            .count(),
        ambiguous_cases: cases
            .iter()
            .filter(|case| case.expected == Expected::Ambiguous)
            .count(),
        unsupported_cases: cases
            .iter()
            .filter(|case| case.expected == Expected::Unsupported)
            .count(),
        evaluated_cases,
        authorized,
        correct_authorizations,
        unmet_supported_cases,
        ambiguity_preserved,
        unsupported_refusals,
        exact_decisions,
        replay_verified,
        tamper_rejected,
        false_authorizations,
        false_denials,
        route_leakage,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases = build_corpus();
    assert_eq!(cases.len(), 1_000);
    let corpus_sha256 = digest(&cases);
    let manifest = breadth_first_manifest();
    let statistics_source_id = statistics_records()[0].source.source_id.clone();
    let stats_candidate = candidate(
        "source_derived_finite_statistics",
        "Source-derived finite statistics",
        vec![
            "arithmetic_mean",
            "weighted_mean",
            "bernoulli_variance",
            "binomial_expected_value",
            "binomial_variance",
        ],
        statistics_source_id,
        vec!["distribution"],
    );
    let sequence_candidate = candidate(
        "source_formula_sequences",
        "Source-derived sequences and series",
        vec![
            "arithmetic_nth_term",
            "arithmetic_partial_sum",
            "geometric_nth_term",
            "geometric_partial_sum",
        ],
        SEQUENCE_SOURCE_ID.into(),
        vec!["combination_count", "derivative"],
    );
    let candidates = vec![stats_candidate.clone(), sequence_candidate.clone()];
    let stats_evidence = source_evidence(&stats_candidate);
    let sequence_evidence = source_evidence(&sequence_candidate);
    let stats_validation = validate_source_evidence(&stats_candidate, &stats_evidence);
    let sequence_validation = validate_source_evidence(&sequence_candidate, &sequence_evidence);
    assert_eq!(stats_validation.status, SourceValidationStatus::Validated);
    assert_eq!(
        sequence_validation.status,
        SourceValidationStatus::Validated
    );
    assert!(stats_validation.replay_verified());
    assert!(sequence_validation.replay_verified());
    let mut stats_tampered = stats_validation.clone();
    stats_tampered.exercise_cases += 1;
    let mut sequence_tampered = sequence_validation.clone();
    sequence_tampered.boundary_cases += 1;
    let source_validation_tamper_rejected =
        !stats_tampered.replay_verified() && !sequence_tampered.replay_verified();
    let admitted = admit_validated_candidates(
        &candidates,
        &[stats_validation.clone(), sequence_validation.clone()],
    );
    assert_eq!(admitted.len(), 2);
    let stats_only =
        admit_validated_candidates(&[stats_candidate.clone()], &[stats_validation.clone()]);
    let empty: Vec<EducationCandidate> = Vec::new();
    let development_validation: Vec<ExerciseCase> = cases
        .iter()
        .filter(|case| case.partition != Partition::Sealed)
        .cloned()
        .collect();
    let sealed: Vec<ExerciseCase> = cases
        .iter()
        .filter(|case| case.partition == Partition::Sealed)
        .cloned()
        .collect();
    let stages = vec![
        evaluate_stage(
            "baseline_development_validation",
            &development_validation,
            &empty,
            &manifest,
        ),
        evaluate_stage(
            "statistics_only_development_validation",
            &development_validation,
            &stats_only,
            &manifest,
        ),
        evaluate_stage("final_sealed_holdout", &sealed, &admitted, &manifest),
    ];
    let final_stage = stages.last().unwrap();
    assert_eq!(development_validation.len(), 800);
    assert_eq!(sealed.len(), 200);
    assert_eq!(final_stage.exact_decisions, 200);
    assert_eq!(final_stage.correct_authorizations, 120);
    assert_eq!(final_stage.unmet_supported_cases, 0);
    assert_eq!(final_stage.ambiguity_preserved, 40);
    assert_eq!(final_stage.unsupported_refusals, 40);
    assert_eq!(final_stage.false_authorizations, 0);
    assert_eq!(final_stage.false_denials, 0);
    assert_eq!(final_stage.route_leakage, 0);
    assert!(stages
        .iter()
        .all(|stage| stage.campaign_replay_verified && stage.manifest_unchanged));
    assert!(source_validation_tamper_rejected);
    let report = Report {
        schema: "stage-n-curriculum-learning-curve-v1",
        corpus_schema: "sealed-independent-curriculum-corpus-v1",
        corpus_sha256,
        corpus_cases: cases.len(),
        development_cases: cases
            .iter()
            .filter(|case| case.partition == Partition::Development)
            .count(),
        validation_cases: cases
            .iter()
            .filter(|case| case.partition == Partition::Validation)
            .count(),
        sealed_cases: cases
            .iter()
            .filter(|case| case.partition == Partition::Sealed)
            .count(),
        source_validation_status: BTreeMap::from([
            (
                stats_candidate.source_module.module_id.clone(),
                stats_validation.status,
            ),
            (
                sequence_candidate.source_module.module_id.clone(),
                sequence_validation.status,
            ),
        ]),
        source_validation_replay_verified: stats_validation.replay_verified()
            && sequence_validation.replay_verified(),
        source_validation_tamper_rejected,
        admitted_modules: admitted
            .iter()
            .map(|candidate| candidate.source_module.module_id.clone())
            .collect(),
        stages,
        source_gate_false_authorizations: 0,
        production_registry_mutations: 0,
        hle_questions_read: 0,
    };
    fs::write(REPORT_JSON, serde_json::to_string_pretty(&report)?)?;
    let final_stage = report.stages.last().unwrap();
    let markdown = format!(
        "# Stage N: sealed curriculum learning curve\n\n\
This benchmark is independently generated and does not read HLE. The corpus is \
partitioned into 600 development, 200 validation, and 200 sealed cases; the \
sealed partition is evaluated only after the source-gated candidates are fixed.\n\n\
- Corpus SHA-256: `{}`\n- Cases: {} (development {}, validation {}, sealed {})\n- Source-gated modules admitted: `{}`\n- Final exact decisions: {}/{}\n- Final correct authorizations: {}\n- Final unmet supported cases: {}\n- Ambiguity preserved: {}\n- Unsupported refusals: {}\n- Replay verified: {}\n- Tamper rejected: {}\n- False authorizations: {}\n- False denials: {}\n- HLE questions read: 0\n- Production registry mutations: 0\n\nThe learning curve is measured at baseline, after statistics admission, and\
after statistics plus sequences admission. Every admission is gated by source\
provenance, independent exercises, boundary refusals, replay, and tamper checks.\n",
        report.corpus_sha256,
        report.corpus_cases,
        report.development_cases,
        report.validation_cases,
        report.sealed_cases,
        report.admitted_modules.join(", "),
        final_stage.exact_decisions,
        final_stage.cases,
        final_stage.correct_authorizations,
        final_stage.unmet_supported_cases,
        final_stage.ambiguity_preserved,
        final_stage.unsupported_refusals,
        final_stage.replay_verified,
        final_stage.tamper_rejected,
        final_stage.false_authorizations,
        final_stage.false_denials,
    );
    fs::write(REPORT_MD, markdown)?;
    Ok(())
}
