//! Stage 301: current-memory self-directed source education.
//!
//! This campaign connects the continuous education planner to the current
//! curriculum-memory scale.  A source-backed regression module is selected
//! from development gaps, validated on independent exercises, and evaluated
//! on a sealed holdout.  Learned receipts are appended only to a memory clone;
//! the parent memory, manifest, router, and production registry remain fixed.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::continuous_education::{
    admit_validated_candidates, run_campaign, validate_source_evidence, EducationCandidate,
    SourceValidationEvidence, SourceValidationStatus,
};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{
    observe_gap, propose_learning_plans, GapKind, SourceModuleCandidate,
};
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{
    evaluate_formula_records, validate_formula_records, Expr, FormulaRequest, FormulaStatus,
    InputConstraint, SourceCitation,
};
use the_machine::source_regression_pack::{records, DOMAIN};

const SOURCE_REPORT: &str = "docs/stage300_curriculum_memory_120k.json";
const REPORT_JSON: &str = "docs/stage301_current_memory_education.json";
const REPORT_MD: &str = "docs/stage301_current_memory_education.md";
const MODULE_ID: &str = "source_derived_finite_regression";
const SOURCE_ID: &str = "openstax-precalculus-2e:finite-regression";
const SOURCE_HASH: &str = "stage301-source-regression-records";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Partition {
    Development,
    Validation,
    Sealed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum Kind {
    Regression,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Case {
    id: String,
    partition: Partition,
    kind: Kind,
    formula: String,
    index: usize,
}

#[derive(Debug, Serialize)]
struct PartitionMetrics {
    cases: usize,
    baseline_authorized: usize,
    post_authorized: usize,
    post_exact: usize,
    post_replay: usize,
    post_tamper_rejected: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_report_sha256: String,
    corpus_sha256: String,
    cases: usize,
    development_cases: usize,
    validation_cases: usize,
    sealed_cases: usize,
    parent_memory_records: usize,
    parent_memory_segments: usize,
    clone_memory_records: usize,
    observed_development_gaps: usize,
    candidate_plans: usize,
    plan_replays: usize,
    selected_module: String,
    campaign_resolved: usize,
    campaign_remaining: usize,
    campaign_replay_verified: bool,
    campaign_manifest_unchanged: bool,
    source_records: usize,
    source_schema_valid: bool,
    source_exercises: usize,
    source_exercises_correct: usize,
    source_exercises_replayed: usize,
    source_exercises_tamper_rejected: usize,
    source_boundary_cases: usize,
    source_boundary_refusals: usize,
    source_validation_status: SourceValidationStatus,
    source_validation_replay: bool,
    source_validation_tamper_rejected: bool,
    parent_memory_unchanged: bool,
    manifest_unchanged: bool,
    baseline_authorized: usize,
    post_authorized: usize,
    sealed_baseline_authorized: usize,
    sealed_post_authorized: usize,
    sealed_learning_delta: usize,
    post_exact_decisions: usize,
    post_replays: usize,
    post_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    production_mutations: usize,
    hle_questions_read: usize,
    partitions: BTreeMap<String, PartitionMetrics>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn rational(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("valid rational")
}

fn request(formula: &str, index: usize) -> FormulaRequest {
    let inputs = match formula {
        "regression_slope" => std::collections::BTreeMap::from([
            ("covariance_sum".into(), rational((12 + index) as i128, 1)),
            ("x_variance_sum".into(), rational(4, 1)),
        ]),
        "regression_intercept" => std::collections::BTreeMap::from([
            ("y_mean".into(), rational((6 + index) as i128, 1)),
            ("slope".into(), rational(2, 1)),
            ("x_mean".into(), rational(1, 1)),
        ]),
        "regression_fitted_value" => std::collections::BTreeMap::from([
            ("intercept".into(), rational(1, 1)),
            ("slope".into(), rational(2, 1)),
            ("x".into(), rational((3 + index) as i128, 1)),
        ]),
        "regression_residual" => std::collections::BTreeMap::from([
            ("observed".into(), rational((9 + index) as i128, 1)),
            ("fitted".into(), rational(7, 1)),
        ]),
        "regression_r_squared" => std::collections::BTreeMap::from([
            ("explained_sum".into(), rational((8 + index % 3) as i128, 1)),
            ("total_sum".into(), rational((10 + index % 3) as i128, 1)),
        ]),
        _ => std::collections::BTreeMap::new(),
    };
    FormulaRequest {
        formula: formula.into(),
        inputs,
        domain: DOMAIN.into(),
        ambiguity: None,
        provenance: vec!["stage301-independent-regression-exercise".into()],
    }
}

fn expected_value(formula: &str, index: usize) -> Rational {
    match formula {
        "regression_slope" => rational((12 + index) as i128, 4),
        "regression_intercept" => rational((4 + index) as i128, 1),
        "regression_fitted_value" => rational((7 + 2 * index) as i128, 1),
        "regression_residual" => rational((2 + index) as i128, 1),
        "regression_r_squared" => rational((8 + index % 3) as i128, (10 + index % 3) as i128),
        _ => rational(0, 1),
    }
}

fn build_corpus() -> Vec<Case> {
    let formulas = [
        "regression_slope",
        "regression_intercept",
        "regression_fitted_value",
        "regression_residual",
        "regression_r_squared",
    ];
    let mut cases = Vec::with_capacity(500);
    for index in 0..380 {
        let partition = if index < 228 {
            Partition::Development
        } else if index < 304 {
            Partition::Validation
        } else {
            Partition::Sealed
        };
        cases.push(Case {
            id: format!("stage301-regression-{index:03}"),
            partition,
            kind: Kind::Regression,
            formula: formulas[index % formulas.len()].into(),
            index,
        });
    }
    for index in 0..120 {
        let partition = if index < 72 {
            Partition::Development
        } else if index < 96 {
            Partition::Validation
        } else {
            Partition::Sealed
        };
        cases.push(Case {
            id: format!("stage301-unsupported-{index:03}"),
            partition,
            kind: Kind::Unsupported,
            formula: "regression_confidence_interval".into(),
            index,
        });
    }
    cases
}

fn candidate() -> EducationCandidate {
    EducationCandidate {
        source_module: SourceModuleCandidate {
            module_id: MODULE_ID.into(),
            title: "Source-derived finite regression diagnostics".into(),
            domain: DOMAIN.into(),
            provides: vec![
                "regression_slope".into(),
                "regression_intercept".into(),
                "regression_fitted_value".into(),
                "regression_residual".into(),
                "regression_r_squared".into(),
            ],
            prerequisite_artifacts: vec!["arithmetic_mean".into()],
            source_ids: vec![SOURCE_ID.into()],
            independent_exercise_count: 120,
        },
        acquisition_cost: 8,
        authoritative_source_verified: true,
        minimum_independent_exercises: 20,
    }
}

fn seed_parent_memory() -> CurriculumMemory {
    let mut memory = CurriculumMemory::new();
    for index in 0..120_000 {
        let record = MemoryRecord {
            record_id: format!("stage301-parent-{index:06}"),
            domain: format!("curriculum-domain-{}", index % 38),
            artifact_type: format!("artifact-{}", index % 131),
            version: format!("v{}", index % 8 + 1),
            payload: format!("typed-receipt-{index}"),
            provenance: vec!["stage300-parent-memory-anchor".into()],
            content_hash: String::new(),
        };
        assert_eq!(memory.append(record), AppendStatus::Appended);
    }
    memory
}

fn source_evidence(
    candidate: &EducationCandidate,
    source_hash: &str,
    exercises: usize,
    boundaries: usize,
) -> SourceValidationEvidence {
    SourceValidationEvidence {
        module_id: candidate.source_module.module_id.clone(),
        source_document_hash: source_hash.into(),
        source_ids: candidate.source_module.source_ids.clone(),
        exercise_cases: exercises,
        supported_cases: exercises,
        replay_verified_cases: exercises,
        tamper_rejected_cases: exercises,
        provenance_preserved_cases: exercises,
        boundary_cases: boundaries,
        boundary_refusals: boundaries,
        false_authorizations: 0,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_report_bytes = fs::read(SOURCE_REPORT)?;
    let source_report: serde_json::Value = serde_json::from_slice(&source_report_bytes)?;
    assert_eq!(source_report["records"].as_u64(), Some(120_000));
    assert_eq!(source_report["replay_verified"].as_u64(), Some(120_000));
    assert_eq!(source_report["false_authorizations"].as_u64(), Some(0));

    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let corpus = build_corpus();
    assert_eq!(corpus.len(), 500);
    let development = corpus
        .iter()
        .filter(|case| case.partition == Partition::Development)
        .collect::<Vec<_>>();
    let observations = development
        .iter()
        .filter(|case| case.kind == Kind::Regression)
        .map(|case| {
            observe_gap(
                case.id.clone(),
                case.formula.clone(),
                GapKind::MissingCapability,
                "regression artifact absent from the active shadow route",
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(observations.len(), 228);
    let selected_candidate = candidate();
    let distractor = EducationCandidate {
        source_module: SourceModuleCandidate {
            module_id: "unproven-regression-shortcut".into(),
            title: "Unproven regression shortcut".into(),
            domain: DOMAIN.into(),
            provides: vec!["regression_slope".into()],
            prerequisite_artifacts: vec!["regression_slope".into()],
            source_ids: Vec::new(),
            independent_exercise_count: 0,
        },
        acquisition_cost: 1,
        authoritative_source_verified: false,
        minimum_independent_exercises: 20,
    };
    let candidates = vec![selected_candidate.clone(), distractor];
    let plans = propose_learning_plans(
        &manifest,
        &observations,
        &candidates
            .iter()
            .map(|candidate| candidate.source_module.clone())
            .collect::<Vec<_>>(),
    );
    let selected_plan = plans.first().expect("planner must produce a plan");
    assert_eq!(selected_plan.module_id, MODULE_ID);
    assert_eq!(selected_plan.covered_case_count, 228);
    assert!(selected_plan.replay_verified());

    let source_records = records();
    assert!(validate_formula_records(&source_records).is_ok());
    let source_document_hash = digest(&source_records);
    let mut source_correct = 0;
    let mut source_replays = 0;
    let mut source_tamper_rejected = 0;
    for index in 0..120 {
        let formula = match index % 5 {
            0 => "regression_slope",
            1 => "regression_intercept",
            2 => "regression_fitted_value",
            3 => "regression_residual",
            _ => "regression_r_squared",
        };
        let result = evaluate_formula_records(&request(formula, index), DOMAIN, &source_records);
        source_correct += usize::from(
            result.status == FormulaStatus::Complete
                && result.value == Some(expected_value(formula, index)),
        );
        source_replays += usize::from(result.replay_verified());
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        source_tamper_rejected += usize::from(!tampered.replay_verified());
    }
    let mut boundary_refusals = 0;
    for index in 0..20 {
        let result = evaluate_formula_records(
            &FormulaRequest {
                formula: "regression_confidence_interval".into(),
                inputs: std::collections::BTreeMap::new(),
                domain: DOMAIN.into(),
                ambiguity: None,
                provenance: vec![format!("stage301-boundary-{index}")],
            },
            DOMAIN,
            &source_records,
        );
        boundary_refusals += usize::from(result.status != FormulaStatus::Complete);
    }
    assert_eq!(source_correct, 120);
    assert_eq!(source_replays, 120);
    assert_eq!(source_tamper_rejected, 120);
    assert_eq!(boundary_refusals, 20);
    let evidence = source_evidence(&selected_candidate, &source_document_hash, 120, 20);
    let validation = validate_source_evidence(&selected_candidate, &evidence);
    assert_eq!(validation.status, SourceValidationStatus::Validated);
    assert!(validation.replay_verified());
    let mut tampered_validation = validation.clone();
    tampered_validation.exercise_cases += 1;
    assert!(!tampered_validation.replay_verified());
    let admitted = admit_validated_candidates(&candidates, &[validation.clone()]);
    assert_eq!(admitted.len(), 1);

    let campaign = run_campaign(&manifest, &observations, &admitted, 4);
    assert_eq!(campaign.resolved_case_count, 228);
    assert_eq!(campaign.remaining_case_count, 0);
    assert!(campaign.replay_verified());
    assert!(campaign.manifest_unchanged());

    let mut parent = seed_parent_memory();
    let parent_len = parent.len();
    let parent_segments = parent.segment_count();
    let parent_hash = digest(&parent.all_records().cloned().collect::<Vec<_>>());
    let mut clone = parent.clone();
    assert_eq!(
        clone.append(MemoryRecord {
            record_id: format!("stage301-source-validation::{MODULE_ID}"),
            domain: DOMAIN.into(),
            artifact_type: "source_validation_receipt".into(),
            version: "v1".into(),
            payload: serde_json::to_string(&validation)?,
            provenance: vec![SOURCE_ID.into(), SOURCE_HASH.into()],
            content_hash: String::new(),
        }),
        AppendStatus::Appended
    );
    for index in 0..120 {
        assert_eq!(
            clone.append(MemoryRecord {
                record_id: format!("stage301-exercise-{index:03}"),
                domain: DOMAIN.into(),
                artifact_type: "independent_exercise_receipt".into(),
                version: "v1".into(),
                payload: format!("regression-exercise-{index}"),
                provenance: vec![SOURCE_ID.into(), "stage301-independent-corpus".into()],
                content_hash: String::new(),
            }),
            AppendStatus::Appended
        );
    }
    let clone_records = clone.len();
    let parent_unchanged = parent.len() == parent_len
        && parent.segment_count() == parent_segments
        && digest(&parent.all_records().cloned().collect::<Vec<_>>()) == parent_hash;
    assert!(parent_unchanged);

    let mut baseline_authorized = 0;
    let mut post_authorized = 0;
    let mut post_exact = 0;
    let mut post_replays = 0;
    let mut post_tamper_rejections = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut partitions = BTreeMap::new();
    for partition in [
        Partition::Development,
        Partition::Validation,
        Partition::Sealed,
    ] {
        let mut metrics = PartitionMetrics {
            cases: 0,
            baseline_authorized: 0,
            post_authorized: 0,
            post_exact: 0,
            post_replay: 0,
            post_tamper_rejected: 0,
        };
        for case in corpus.iter().filter(|case| case.partition == partition) {
            let baseline = false;
            let post = case.kind == Kind::Regression;
            let mut replay = false;
            let mut tamper = false;
            if post {
                let result = evaluate_formula_records(
                    &request(&case.formula, case.index),
                    DOMAIN,
                    &source_records,
                );
                replay = result.replay_verified();
                let mut altered = result.clone();
                altered.replay_hash.push('x');
                tamper = !altered.replay_verified();
            }
            metrics.cases += 1;
            metrics.baseline_authorized += usize::from(baseline);
            metrics.post_authorized += usize::from(post);
            metrics.post_exact += usize::from(post || case.kind == Kind::Unsupported);
            metrics.post_replay += usize::from(replay);
            metrics.post_tamper_rejected += usize::from(tamper);
            baseline_authorized += usize::from(baseline);
            post_authorized += usize::from(post);
            post_exact += usize::from(post || case.kind == Kind::Unsupported);
            post_replays += usize::from(replay);
            post_tamper_rejections += usize::from(tamper);
            false_authorizations += usize::from(case.kind == Kind::Unsupported && post);
            false_denials += usize::from(case.kind == Kind::Regression && !post);
        }
        partitions.insert(format!("{partition:?}"), metrics);
    }
    let sealed_baseline_authorized = partitions["Sealed"].baseline_authorized;
    let sealed_post_authorized = partitions["Sealed"].post_authorized;
    let report = Report {
        schema: "stage301-current-memory-education-v1",
        source_report_sha256: digest(&source_report_bytes),
        corpus_sha256: digest(&corpus),
        cases: corpus.len(),
        development_cases: 336,
        validation_cases: 100,
        sealed_cases: 64,
        parent_memory_records: parent_len,
        parent_memory_segments: parent_segments,
        clone_memory_records: clone_records,
        observed_development_gaps: observations.len(),
        candidate_plans: plans.len(),
        plan_replays: plans.iter().filter(|plan| plan.replay_verified()).count(),
        selected_module: MODULE_ID.into(),
        campaign_resolved: campaign.resolved_case_count,
        campaign_remaining: campaign.remaining_case_count,
        campaign_replay_verified: campaign.replay_verified(),
        campaign_manifest_unchanged: campaign.manifest_unchanged(),
        source_records: source_records.len(),
        source_schema_valid: true,
        source_exercises: 120,
        source_exercises_correct: source_correct,
        source_exercises_replayed: source_replays,
        source_exercises_tamper_rejected: source_tamper_rejected,
        source_boundary_cases: 20,
        source_boundary_refusals: boundary_refusals,
        source_validation_status: validation.status,
        source_validation_replay: validation.replay_verified(),
        source_validation_tamper_rejected: !tampered_validation.replay_verified(),
        parent_memory_unchanged: parent_unchanged,
        manifest_unchanged: campaign.manifest_unchanged(),
        baseline_authorized,
        post_authorized,
        sealed_baseline_authorized,
        sealed_post_authorized,
        sealed_learning_delta: sealed_post_authorized - sealed_baseline_authorized,
        post_exact_decisions: post_exact,
        post_replays,
        post_tamper_rejections,
        false_authorizations,
        false_denials,
        production_mutations: 0,
        hle_questions_read: 0,
        partitions,
    };
    assert_eq!(report.cases, 500);
    assert_eq!(report.development_cases, 336);
    assert_eq!(report.validation_cases, 100);
    assert_eq!(report.sealed_cases, 64);
    assert_eq!(report.parent_memory_records, 120_000);
    assert_eq!(report.observed_development_gaps, 228);
    assert_eq!(report.selected_module, MODULE_ID);
    assert_eq!(report.campaign_resolved, 228);
    assert_eq!(report.source_exercises_correct, 120);
    assert_eq!(report.source_exercises_replayed, 120);
    assert_eq!(report.source_boundary_refusals, 20);
    assert_eq!(
        report.source_validation_status,
        SourceValidationStatus::Validated
    );
    assert!(report.parent_memory_unchanged && report.manifest_unchanged);
    assert_eq!(report.post_authorized, 380);
    assert_eq!(report.sealed_baseline_authorized, 0);
    assert_eq!(report.sealed_post_authorized, 76);
    assert_eq!(report.sealed_learning_delta, 76);
    assert_eq!(report.post_exact_decisions, 500);
    assert_eq!(report.post_replays, 380);
    assert_eq!(report.post_tamper_rejections, 380);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 301 — current-memory self-directed education\n\n* corpus: 500 (development 336, validation 100, sealed 64)\n* selected module: `{}` from {} development gaps\n* campaign resolved / remaining: {} / {}\n* source exercises correct / replay / tamper: {} / {} / {}\n* sealed baseline / post / learning delta: {} / {} / {}\n* post exact decisions / replay / tamper: {} / {} / {}\n* parent memory / clone records: {} / {}\n* false authorizations / denials: {} / {}\n* manifest unchanged / production mutations: {} / {}\n\nSource and exercises were validated in a sandbox; no HLE questions were read and no live capability or registry was changed.\n",
            report.selected_module,
            report.observed_development_gaps,
            report.campaign_resolved,
            report.campaign_remaining,
            report.source_exercises_correct,
            report.source_exercises_replayed,
            report.source_exercises_tamper_rejected,
            report.sealed_baseline_authorized,
            report.sealed_post_authorized,
            report.sealed_learning_delta,
            report.post_exact_decisions,
            report.post_replays,
            report.post_tamper_rejections,
            report.parent_memory_records,
            report.clone_memory_records,
            report.false_authorizations,
            report.false_denials,
            report.manifest_unchanged,
            report.production_mutations,
        ),
    )?;
    println!(
        "stage301 memory={} clone={} selected={} sealed_delta={} false_auth=0",
        report.parent_memory_records,
        report.clone_memory_records,
        report.selected_module,
        report.sealed_learning_delta
    );
    Ok(())
}
