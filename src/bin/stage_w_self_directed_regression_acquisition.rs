//! Stage W: self-directed selection and sandbox validation of source regression.
//!
//! The planner sees only replayable typed gaps and candidate metadata.  It
//! selects the source-derived regression module by exact artifact coverage,
//! then validates its attributed catalog and independent exercises in a
//! sandbox.  The curriculum manifest and production routing remain unchanged.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{
    candidate_is_promotable, cluster_gaps, manifest_unchanged, observe_gap,
    propose_learning_plans, GapKind, SourceModuleCandidate,
};
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{
    validate_formula_records, Expr, InputConstraint, SourceCitation,
};
use the_machine::source_regression_pack::{evaluate_regression, records, DOMAIN};
use the_machine::source_formula_pack::{FormulaRequest, FormulaStatus};

const SOURCE: &str = include_str!("../../docs/sources/openstax_finite_regression_source.txt");
const REPORT_JSON: &str = "docs/stage_w_self_directed_regression_acquisition.json";
const REPORT_MD: &str = "docs/stage_w_self_directed_regression_acquisition.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    observed_cases: usize,
    gap_clusters: usize,
    candidate_plans: usize,
    selected_module: String,
    selected_coverage: usize,
    selected_plan_replay: bool,
    plan_tamper_rejected: bool,
    source_document_sha256: String,
    extracted_records: usize,
    independent_validation_cases: usize,
    independent_validation_correct: usize,
    independent_replay_verified: usize,
    independent_tamper_rejected: usize,
    source_mutations: usize,
    source_mutations_rejected: usize,
    shadow_promotable: bool,
    manifest_unchanged: bool,
    blocked_shortcuts: usize,
    false_authorizations: usize,
    production_authorizations: usize,
    corpus_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(n: i128, d: i128) -> Rational {
    Rational::new(n, d).expect("valid rational")
}

fn request(formula: &str, index: usize) -> FormulaRequest {
    let inputs = match formula {
        "regression_slope" => BTreeMap::from([
            ("covariance_sum".into(), q((12 + index) as i128, 1)),
            ("x_variance_sum".into(), q(4, 1)),
        ]),
        "regression_intercept" => BTreeMap::from([
            ("y_mean".into(), q((6 + index) as i128, 1)),
            ("slope".into(), q(2, 1)),
            ("x_mean".into(), q(1, 1)),
        ]),
        "regression_fitted_value" => BTreeMap::from([
            ("intercept".into(), q(1, 1)),
            ("slope".into(), q(2, 1)),
            ("x".into(), q((3 + index) as i128, 1)),
        ]),
        "regression_residual" => BTreeMap::from([
            ("observed".into(), q((9 + index) as i128, 1)),
            ("fitted".into(), q(7, 1)),
        ]),
        "regression_r_squared" => BTreeMap::from([
            ("explained_sum".into(), q(8 + (index % 3) as i128, 1)),
            ("total_sum".into(), q(10 + (index % 3) as i128, 1)),
        ]),
        _ => BTreeMap::new(),
    };
    FormulaRequest {
        formula: formula.into(),
        inputs,
        domain: DOMAIN.into(),
        ambiguity: None,
        provenance: vec!["stage-w-independent-regression-exercise".into()],
    }
}

fn expected_value(formula: &str, index: usize) -> Rational {
    match formula {
        "regression_slope" => q((12 + index) as i128, 4),
        "regression_intercept" => q((4 + index) as i128, 1),
        "regression_fitted_value" => q((7 + 2 * index) as i128, 1),
        "regression_residual" => q((2 + index) as i128, 1),
        "regression_r_squared" => q(8 + (index % 3) as i128, 10 + (index % 3) as i128),
        _ => q(0, 1),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let mut observations = Vec::with_capacity(500);
    for index in 0..220 {
        observations.push(observe_gap(
            format!("regression_slope_gap_{index:03}"),
            "regression_slope",
            GapKind::MissingCapability,
            "bounded regression slope artifact is absent from the active route",
        ));
    }
    for index in 0..100 {
        observations.push(observe_gap(
            format!("regression_intercept_gap_{index:03}"),
            "regression_intercept",
            GapKind::MissingCapability,
            "bounded regression intercept artifact is absent from the active route",
        ));
    }
    for index in 0..60 {
        observations.push(observe_gap(
            format!("regression_fitted_gap_{index:03}"),
            "regression_fitted_value",
            GapKind::MissingCapability,
            "fitted-value artifact is required downstream",
        ));
    }
    for index in 0..60 {
        observations.push(observe_gap(
            format!("statistics_gap_{index:03}"),
            "arithmetic_mean",
            GapKind::MissingCapability,
            "statistics prerequisite is a competing source candidate",
        ));
    }
    for index in 0..60 {
        observations.push(observe_gap(
            format!("unsupported_gap_{index:03}"),
            "regression_confidence_interval",
            GapKind::Unsupported,
            "uncertainty interval is outside the bounded source catalog",
        ));
    }
    assert_eq!(observations.len(), 500);
    let candidates = vec![
        SourceModuleCandidate {
            module_id: "source_derived_finite_regression".into(),
            title: "Source-derived finite regression diagnostics".into(),
            domain: "finite_statistics.regression".into(),
            provides: vec![
                "regression_slope".into(),
                "regression_intercept".into(),
                "regression_fitted_value".into(),
                "regression_residual".into(),
                "regression_r_squared".into(),
            ],
            prerequisite_artifacts: vec!["arithmetic_mean".into()],
            source_ids: vec!["openstax-precalculus-2e:finite-regression".into()],
            independent_exercise_count: 240,
        },
        SourceModuleCandidate {
            module_id: "source_derived_finite_statistics".into(),
            title: "Existing finite statistics catalog".into(),
            domain: "finite_statistics".into(),
            provides: vec!["arithmetic_mean".into()],
            prerequisite_artifacts: vec!["arithmetic_mean".into()],
            source_ids: vec!["openstax-precalculus-2e:sequences-series".into()],
            independent_exercise_count: 240,
        },
        SourceModuleCandidate {
            module_id: "unproven_regression_shortcut".into(),
            title: "Unproven regression shortcut".into(),
            domain: "finite_statistics.regression".into(),
            provides: vec!["regression_slope".into()],
            prerequisite_artifacts: vec!["regression_slope".into()],
            source_ids: Vec::new(),
            independent_exercise_count: 0,
        },
    ];
    let plans = propose_learning_plans(&manifest, &observations, &candidates);
    let selected = plans.first().expect("planner returns candidates");
    assert_eq!(selected.module_id, "source_derived_finite_regression");
    assert_eq!(selected.covered_case_count, 380);
    let selected_plan_replay = selected.replay_verified();
    let mut tampered_plan = selected.clone();
    tampered_plan.covered_case_count += 1;
    let plan_tamper_rejected = !tampered_plan.replay_verified();

    let source_document_sha256 = digest(&SOURCE);
    let source_records = records();
    let mut independent_validation_correct = 0;
    let mut independent_replay_verified = 0;
    let mut independent_tamper_rejected = 0;
    let formulas = [
        "regression_slope",
        "regression_intercept",
        "regression_fitted_value",
        "regression_residual",
        "regression_r_squared",
    ];
    for index in 0..120 {
        let formula = formulas[index % formulas.len()];
        let result = evaluate_regression(&request(formula, index));
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        if result.status == FormulaStatus::Complete
            && result.value == Some(expected_value(formula, index))
        {
            independent_validation_correct += 1;
        }
        if result.replay_verified() { independent_replay_verified += 1; }
        if !tampered.replay_verified() { independent_tamper_rejected += 1; }
    }
    let mut mutations = Vec::new();
    let mut duplicate_id = source_records.clone();
    duplicate_id[1].formula_id = duplicate_id[0].formula_id.clone();
    mutations.push(duplicate_id);
    let mut undeclared_input = source_records.clone();
    undeclared_input[0].expression = Expr::Input("unlisted".into());
    mutations.push(undeclared_input);
    let mut duplicate_alias = source_records.clone();
    let duplicate_alias_value = duplicate_alias[0].aliases[0].clone();
    duplicate_alias[1].aliases.push(duplicate_alias_value);
    mutations.push(duplicate_alias);
    let mut undeclared_constraint = source_records.clone();
    undeclared_constraint[0]
        .constraints
        .push(InputConstraint::Positive("unlisted".into()));
    mutations.push(undeclared_constraint);
    let mut bad_citation = source_records.clone();
    bad_citation[0].source = SourceCitation { source_id: String::new(), ..bad_citation[0].source.clone() };
    mutations.push(bad_citation);
    let mut malformed_expression = source_records.clone();
    malformed_expression[0].expression = Expr::Div(
        Box::new(Expr::Input("covariance_sum".into())),
        Box::new(Expr::Input("not_declared".into())),
    );
    mutations.push(malformed_expression);
    let source_mutations_rejected = mutations
        .iter()
        .filter(|mutation| validate_formula_records(mutation).is_err())
        .count();
    let shadow_promotable = candidate_is_promotable(selected, 120)
        && independent_validation_correct == 120
        && independent_replay_verified == 120
        && independent_tamper_rejected == 120;
    let blocked_shortcuts = plans
        .iter()
        .filter(|plan| plan.status == the_machine::curriculum_campaign::PlanStatus::Blocked)
        .count();
    let false_authorizations = plans
        .iter()
        .filter(|plan| plan.status == the_machine::curriculum_campaign::PlanStatus::Proposed)
        .filter(|plan| plan.source_ids.is_empty() || plan.independent_exercise_count == 0)
        .count();
    assert_eq!(source_records.len(), 5);
    assert_eq!(source_mutations_rejected, 6);
    assert_eq!(independent_validation_correct, 120);
    assert_eq!(independent_replay_verified, 120);
    assert_eq!(independent_tamper_rejected, 120);
    assert_eq!(blocked_shortcuts, 1);
    assert_eq!(false_authorizations, 0);
    assert!(shadow_promotable);
    assert!(selected_plan_replay);
    assert!(plan_tamper_rejected);
    assert!(manifest_unchanged(&manifest_hash, &manifest));
    let report = Report {
        schema: "stage-w-self-directed-regression-acquisition-v1",
        observed_cases: observations.len(),
        gap_clusters: cluster_gaps(&observations).len(),
        candidate_plans: plans.len(),
        selected_module: selected.module_id.clone(),
        selected_coverage: selected.covered_case_count,
        selected_plan_replay,
        plan_tamper_rejected,
        source_document_sha256,
        extracted_records: source_records.len(),
        independent_validation_cases: 120,
        independent_validation_correct,
        independent_replay_verified,
        independent_tamper_rejected,
        source_mutations: mutations.len(),
        source_mutations_rejected,
        shadow_promotable,
        manifest_unchanged: manifest_unchanged(&manifest_hash, &manifest),
        blocked_shortcuts,
        false_authorizations,
        production_authorizations: 0,
        corpus_sha256: digest(&(observations, candidates, plans)),
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    fs::write(REPORT_MD, format!("# Stage W: self-directed source regression acquisition\n\n- Observed typed gaps: 500\n- Gap clusters: {}\n- Selected module: `{}` (exact coverage {})\n- Candidate plans: {}\n- Independent validation: 120/120\n- Replay and tamper: 120/120 each\n- Source mutations rejected: 6/6\n- Shadow promotable: {}\n- Manifest unchanged: {}\n- Blocked shortcuts: {}\n- False authorizations: 0\n- Production authorizations: 0\n- HLE questions read: 0\n- Corpus report: `{}`\n", report.gap_clusters, report.selected_module, report.selected_coverage, report.candidate_plans, report.shadow_promotable, report.manifest_unchanged, report.blocked_shortcuts, REPORT_JSON))?;
    println!("{serialized}");
    Ok(())
}
