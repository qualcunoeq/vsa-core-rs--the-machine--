//! Stage D source-derived finite-regression campaign.
//!
//! The subject formulas live only in the attributed source transcription. The
//! benchmark uses the domain-agnostic source catalog parser/interpreter and
//! keeps the catalog shadow-only.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{
    evaluate_formula_records, validate_formula_records, Expr, FormulaRequest, FormulaResult,
    FormulaStatus, InputConstraint, SourceCitation,
};
use the_machine::source_regression_pack::{records, DOMAIN};

#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
enum Expected {
    Complete,
    Ambiguous,
    Refused,
}

#[derive(Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    actual: FormulaStatus,
    expected_value: Option<Rational>,
    actual_value: Option<Rational>,
    exact: bool,
    value_correct: bool,
    source_preserved: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    catalog_sha256: String,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_values: usize,
    source_preserved: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    mutation_cases: usize,
    mutation_rejections: usize,
    generated_exercises: usize,
    generated_exercises_complete: usize,
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<Receipt>,
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("valid benchmark rational")
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("regression benchmark serializes"))
    )
}

fn inputs(formula: &str) -> BTreeMap<String, Rational> {
    match formula {
        "regression_slope" => BTreeMap::from([
            ("covariance_sum".into(), q(12, 1)),
            ("x_variance_sum".into(), q(4, 1)),
        ]),
        "regression_intercept" => BTreeMap::from([
            ("y_mean".into(), q(7, 1)),
            ("slope".into(), q(3, 1)),
            ("x_mean".into(), q(2, 1)),
        ]),
        "regression_fitted_value" => BTreeMap::from([
            ("intercept".into(), q(1, 1)),
            ("slope".into(), q(3, 1)),
            ("x".into(), q(4, 1)),
        ]),
        "regression_residual" => {
            BTreeMap::from([("observed".into(), q(15, 1)), ("fitted".into(), q(13, 1))])
        }
        "regression_r_squared" => BTreeMap::from([
            ("explained_sum".into(), q(12, 1)),
            ("total_sum".into(), q(16, 1)),
        ]),
        _ => BTreeMap::new(),
    }
}

fn expected_value(formula: &str) -> Option<Rational> {
    Some(match formula {
        "regression_slope" => q(3, 1),
        "regression_intercept" => q(1, 1),
        "regression_fitted_value" => q(13, 1),
        "regression_residual" => q(2, 1),
        "regression_r_squared" => q(3, 4),
        _ => return None,
    })
}

fn request(formula: &str) -> FormulaRequest {
    FormulaRequest {
        formula: formula.into(),
        inputs: inputs(formula),
        domain: DOMAIN.into(),
        ambiguity: None,
        provenance: vec!["stage-d-independent-regression-corpus".into()],
    }
}

fn run(
    id: String,
    expected: Expected,
    request: FormulaRequest,
    expected_value: Option<Rational>,
    catalog: &[the_machine::source_formula_pack::FormulaRecord],
) -> Receipt {
    let result: FormulaResult = evaluate_formula_records(&request, DOMAIN, catalog);
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    let exact = match expected {
        Expected::Complete => result.status == FormulaStatus::Complete,
        Expected::Ambiguous => result.status == FormulaStatus::Ambiguous,
        Expected::Refused => result.status != FormulaStatus::Complete,
    };
    let value_correct = expected != Expected::Complete || result.value == expected_value;
    Receipt {
        id,
        expected,
        actual: result.status,
        expected_value,
        actual_value: result.value.clone(),
        exact,
        value_correct,
        source_preserved: expected == Expected::Complete && result.source.is_some(),
        replay_verified: result.replay_verified(),
        tamper_rejected: !tampered.replay_verified(),
        false_authorization: expected != Expected::Complete
            && result.status == FormulaStatus::Complete,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = records();
    let catalog_sha256 = digest(&catalog);
    let formulas = [
        "regression_slope",
        "regression_intercept",
        "regression_fitted_value",
        "regression_residual",
        "regression_r_squared",
    ];
    let mut receipts = Vec::with_capacity(240);
    for index in 0..120 {
        let formula = formulas[index % formulas.len()];
        receipts.push(run(
            format!("supported_{index:03}"),
            Expected::Complete,
            request(formula),
            expected_value(formula),
            &catalog,
        ));
    }
    for index in 0..40 {
        let formula = formulas[index % formulas.len()];
        let mut ambiguous = request(formula);
        ambiguous.ambiguity =
            Some("the source wording does not select one regression convention".into());
        receipts.push(run(
            format!("ambiguous_{index:03}"),
            Expected::Ambiguous,
            ambiguous,
            expected_value(formula),
            &catalog,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("unknown_formula_{index:03}"),
            Expected::Refused,
            request("unsupported_regression_claim"),
            None,
            &catalog,
        ));
    }
    for index in 0..20 {
        let mut missing = request("regression_slope");
        missing.inputs.remove("x_variance_sum");
        receipts.push(run(
            format!("missing_input_{index:03}"),
            Expected::Refused,
            missing,
            None,
            &catalog,
        ));
    }
    for index in 0..20 {
        let mut degenerate = request("regression_slope");
        degenerate.inputs.insert("x_variance_sum".into(), q(0, 1));
        receipts.push(run(
            format!("zero_x_variation_{index:03}"),
            Expected::Refused,
            degenerate,
            None,
            &catalog,
        ));
    }
    for index in 0..10 {
        let mut invalid_domain = request("regression_r_squared");
        invalid_domain.domain = "unbounded_regression_theory".into();
        receipts.push(run(
            format!("unsupported_domain_{index:03}"),
            Expected::Refused,
            invalid_domain,
            None,
            &catalog,
        ));
    }
    for index in 0..10 {
        let mut zero_total = request("regression_r_squared");
        zero_total.inputs.insert("total_sum".into(), q(0, 1));
        receipts.push(run(
            format!("zero_total_variation_{index:03}"),
            Expected::Refused,
            zero_total,
            None,
            &catalog,
        ));
    }
    assert_eq!(receipts.len(), 240);

    let mut mutations = Vec::new();
    let mut duplicate_id = catalog.clone();
    duplicate_id[1].formula_id = duplicate_id[0].formula_id.clone();
    mutations.push(duplicate_id);
    let mut undeclared_input = catalog.clone();
    undeclared_input[0].expression = Expr::Input("unlisted".into());
    mutations.push(undeclared_input);
    let mut duplicate_alias = catalog.clone();
    let duplicate_alias_value = duplicate_alias[0].aliases[0].clone();
    duplicate_alias[1].aliases.push(duplicate_alias_value);
    mutations.push(duplicate_alias);
    let mut undeclared_constraint = catalog.clone();
    undeclared_constraint[0]
        .constraints
        .push(InputConstraint::Positive("unlisted".into()));
    mutations.push(undeclared_constraint);
    let mut bad_citation = catalog.clone();
    bad_citation[0].source = SourceCitation {
        source_id: String::new(),
        ..bad_citation[0].source.clone()
    };
    mutations.push(bad_citation);
    let mut malformed_expression = catalog.clone();
    malformed_expression[0].expression = Expr::Div(
        Box::new(Expr::Input("covariance_sum".into())),
        Box::new(Expr::Input("not_declared".into())),
    );
    mutations.push(malformed_expression);
    let mutation_rejections = mutations
        .iter()
        .filter(|candidate| validate_formula_records(candidate).is_err())
        .count();

    let generated_exercises = catalog.len();
    let generated_exercises_complete = catalog
        .iter()
        .filter(|record| {
            let result = evaluate_formula_records(
                &FormulaRequest {
                    formula: record.formula_id.clone(),
                    inputs: generated_inputs(record),
                    domain: DOMAIN.into(),
                    ambiguity: None,
                    provenance: vec!["source-constraint-exercise-generator".into()],
                },
                DOMAIN,
                &catalog,
            );
            result.status == FormulaStatus::Complete && result.replay_verified()
        })
        .count();

    let supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Complete)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|r| r.expected == Expected::Ambiguous)
        .count();
    let refused = receipts
        .iter()
        .filter(|r| r.expected == Expected::Refused)
        .count();
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let supported_values = receipts
        .iter()
        .filter(|r| r.expected == Expected::Complete && r.value_correct)
        .count();
    let source_preserved = receipts.iter().filter(|r| r.source_preserved).count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.expected == Expected::Complete && !r.exact)
        .count();
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!(
        (
            exact_decisions,
            supported_values,
            source_preserved,
            replay_verified,
            tamper_rejections,
            mutation_rejections,
            generated_exercises_complete,
            false_authorizations,
            false_denials,
        ),
        (240, 120, 120, 240, 240, 6, 5, 0, 0)
    );
    let report = Report {
        schema: "stage-d-source-derived-finite-regression-v1",
        source: "OpenStax Introductory Statistics 2e, the regression equation",
        catalog_sha256,
        corpus_sha256: digest(&receipts),
        cases: receipts.len(),
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_values,
        source_preserved,
        replay_verified,
        tamper_rejections,
        mutation_cases: mutations.len(),
        mutation_rejections,
        generated_exercises,
        generated_exercises_complete,
        false_authorizations,
        false_denials,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage_d_source_regression_pack.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}

fn generated_inputs(
    record: &the_machine::source_formula_pack::FormulaRecord,
) -> BTreeMap<String, Rational> {
    let mut inputs = BTreeMap::new();
    for name in &record.required_inputs {
        inputs.insert(name.clone(), q(2, 1));
    }
    for constraint in &record.constraints {
        let (name, value) = match constraint {
            InputConstraint::Positive(name) => (name, q(3, 1)),
            InputConstraint::PositiveInteger(name) => (name, q(4, 1)),
            InputConstraint::NonnegativeInteger(name) => (name, q(4, 1)),
            InputConstraint::Probability(name) => (name, q(1, 4)),
            InputConstraint::NotEqualInteger(name, forbidden) => (name, q(forbidden + 1, 1)),
        };
        inputs.insert(name.clone(), value);
    }
    inputs
}
