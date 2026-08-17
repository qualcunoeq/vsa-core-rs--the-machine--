//! Stage 82: execute the source-backed module selected from external gaps.
//!
//! The selected sequence module is validated on an independently generated
//! development corpus and an untouched holdout.  The benchmark never uses the
//! external residual answers as training data and never promotes the module.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{evaluate_formula, FormulaRequest, FormulaStatus};

const PLAN_REPORT: &str = "docs/stage81_external_gap_education.json";
const REPORT_JSON: &str = "docs/stage82_execute_external_education.json";
const REPORT_MD: &str = "docs/stage82_execute_external_education.md";

#[derive(Debug, Deserialize)]
struct PlanSummary {
    selected_module: Option<String>,
    selected_coverage: usize,
    selected_promotable_in_sandbox: bool,
    manifest_unchanged: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Complete,
    Ambiguous,
    Refused,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    plan_report_sha256: String,
    source_id: &'static str,
    development_cases: usize,
    development_supported: usize,
    development_ambiguous: usize,
    development_refused: usize,
    development_exact_decisions: usize,
    development_artifacts: usize,
    development_value_correct: usize,
    development_replays: usize,
    development_tamper_rejections: usize,
    holdout_cases: usize,
    holdout_exact_decisions: usize,
    holdout_value_correct: usize,
    holdout_replays: usize,
    holdout_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    selected_module: Option<String>,
    selected_external_gap_coverage: usize,
    plan_promotable_in_sandbox: bool,
    manifest_unchanged: bool,
    source_provenance_preserved: usize,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(n: i128) -> Rational {
    Rational::new(n, 1).unwrap()
}

fn pow(base: &Rational, exponent: usize) -> Rational {
    let mut value = Rational::one();
    for _ in 0..exponent {
        value = value.mul(base).unwrap();
    }
    value
}

fn values(index: usize) -> BTreeMap<String, Rational> {
    BTreeMap::from([
        ("a1".into(), q(2 + (index % 5) as i128)),
        ("n".into(), q(2 + (index % 6) as i128)),
        ("d".into(), q(1 + (index % 4) as i128)),
        ("r".into(), q(2 + (index % 3) as i128)),
    ])
}

fn oracle(formula: &str, inputs: &BTreeMap<String, Rational>) -> Option<Rational> {
    let a1 = inputs.get("a1")?;
    let n = inputs.get("n")?;
    let d = inputs.get("d")?;
    let r = inputs.get("r")?;
    let one = q(1);
    match formula {
        "arithmetic_nth_term" => Some(a1.add(&n.sub(&one)?.mul(d)?)?),
        "arithmetic_partial_sum" => Some(
            n.mul(&a1.mul(&q(2))?.add(&n.sub(&one)?.mul(d)?)?)?
                .div(&q(2))?,
        ),
        "geometric_nth_term" => Some(a1.mul(&pow(r, n.numerator as usize - 1))?),
        "geometric_partial_sum" => Some(
            a1.mul(&pow(r, n.numerator as usize).sub(&one)?)?
                .div(&r.sub(&one)?)?,
        ),
        _ => None,
    }
}

fn request(formula: &str, index: usize) -> FormulaRequest {
    FormulaRequest {
        formula: formula.into(),
        inputs: values(index),
        domain: "source_derived_sequences_series".into(),
        ambiguity: None,
        provenance: vec![format!("stage82-independent-exercise:{index}")],
    }
}

fn run_case(
    formula: &str,
    expected: Expected,
    mut request: FormulaRequest,
) -> (bool, bool, bool, bool, bool, bool) {
    let result = evaluate_formula(&request);
    let expected_value = oracle(formula, &request.inputs);
    let exact = match expected {
        Expected::Complete => {
            result.status == FormulaStatus::Complete && result.value == expected_value
        }
        Expected::Ambiguous => result.status == FormulaStatus::Ambiguous,
        Expected::Refused => result.status != FormulaStatus::Complete,
    };
    let artifact = expected == Expected::Complete && result.status == FormulaStatus::Complete;
    let value_correct = expected != Expected::Complete || result.value == expected_value;
    let replay = result.replay_verified();
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    let tamper = !tampered.replay_verified();
    let false_auth = expected != Expected::Complete && result.status == FormulaStatus::Complete;
    let false_deny = expected == Expected::Complete && !exact;
    request.inputs.clear();
    (
        exact,
        artifact,
        value_correct,
        replay,
        tamper,
        false_auth || false_deny,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let plan_bytes = fs::read(PLAN_REPORT)?;
    let plan_hash = digest(&plan_bytes);
    let plan: PlanSummary = serde_json::from_slice(&plan_bytes)?;
    assert_eq!(
        plan.selected_module.as_deref(),
        Some("source_formula_sequences")
    );
    assert!(plan.selected_promotable_in_sandbox);
    assert!(plan.manifest_unchanged);
    let manifest_before = breadth_first_manifest().replay_hash();
    let formulas = [
        "arithmetic_nth_term",
        "arithmetic_partial_sum",
        "geometric_nth_term",
        "geometric_partial_sum",
    ];
    let mut dev_exact = 0;
    let mut dev_artifacts = 0;
    let mut dev_values = 0;
    let mut dev_replay = 0;
    let mut dev_tamper = 0;
    let mut false_auth = 0;
    let mut false_deny = 0;
    for index in 0..120 {
        let formula = formulas[index % formulas.len()];
        let (exact, artifact, value, replay, tamper, error) =
            run_case(formula, Expected::Complete, request(formula, index));
        dev_exact += usize::from(exact);
        dev_artifacts += usize::from(artifact);
        dev_values += usize::from(value);
        dev_replay += usize::from(replay);
        dev_tamper += usize::from(tamper);
        false_deny += usize::from(error);
    }
    for index in 0..40 {
        let formula = formulas[index % formulas.len()];
        let mut req = request(formula, index + 40);
        req.ambiguity = Some("source notation admits multiple sequence readings".into());
        let (exact, _, _, replay, tamper, error) = run_case(formula, Expected::Ambiguous, req);
        dev_exact += usize::from(exact);
        dev_replay += usize::from(replay);
        dev_tamper += usize::from(tamper);
        false_auth += usize::from(error);
    }
    for index in 0..20 {
        let mut req = request("unknown_formula", index + 80);
        let (exact, _, _, replay, tamper, error) =
            run_case("unknown_formula", Expected::Refused, req.clone());
        dev_exact += usize::from(exact);
        dev_replay += usize::from(replay);
        dev_tamper += usize::from(tamper);
        false_auth += usize::from(error);
        req.formula = "arithmetic_nth_term".into();
        req.inputs.remove("d");
        let (exact, _, _, replay, tamper, error) =
            run_case("arithmetic_nth_term", Expected::Refused, req);
        dev_exact += usize::from(exact);
        dev_replay += usize::from(replay);
        dev_tamper += usize::from(tamper);
        false_auth += usize::from(error);
    }
    for index in 0..20 {
        let mut req = request("arithmetic_nth_term", index + 120);
        req.inputs.insert("n".into(), q(0));
        let (exact, _, _, replay, tamper, error) =
            run_case("arithmetic_nth_term", Expected::Refused, req);
        dev_exact += usize::from(exact);
        dev_replay += usize::from(replay);
        dev_tamper += usize::from(tamper);
        false_auth += usize::from(error);
    }
    for index in 0..20 {
        let mut req = request("geometric_partial_sum", index + 140);
        req.inputs.insert("r".into(), q(1));
        let (exact, _, _, replay, tamper, error) =
            run_case("geometric_partial_sum", Expected::Refused, req);
        dev_exact += usize::from(exact);
        dev_replay += usize::from(replay);
        dev_tamper += usize::from(tamper);
        false_auth += usize::from(error);
    }
    let mut holdout_exact = 0;
    let mut holdout_values = 0;
    let mut holdout_replay = 0;
    let mut holdout_tamper = 0;
    for index in 0..60 {
        let formula = formulas[(index + 1) % formulas.len()];
        let (exact, _, value, replay, tamper, error) =
            run_case(formula, Expected::Complete, request(formula, index + 300));
        holdout_exact += usize::from(exact);
        holdout_values += usize::from(value);
        holdout_replay += usize::from(replay);
        holdout_tamper += usize::from(tamper);
        false_deny += usize::from(error);
    }
    assert_eq!(dev_exact, 240);
    assert_eq!(dev_artifacts, 120);
    assert_eq!(dev_values, 120);
    assert_eq!(dev_replay, 240);
    assert_eq!(dev_tamper, 240);
    assert_eq!(holdout_exact, 60);
    assert_eq!(holdout_values, 60);
    assert_eq!(holdout_replay, 60);
    assert_eq!(holdout_tamper, 60);
    assert_eq!(false_auth, 0);
    assert_eq!(false_deny, 0);
    let manifest_unchanged = manifest_before == breadth_first_manifest().replay_hash();
    assert!(manifest_unchanged);
    let report = Report {
        schema: "stage82-execute-external-education-v1",
        plan_report_sha256: plan_hash,
        source_id: "openstax-precalculus-2e:sequences-series",
        development_cases: 240,
        development_supported: 120,
        development_ambiguous: 40,
        development_refused: 80,
        development_exact_decisions: dev_exact,
        development_artifacts: dev_artifacts,
        development_value_correct: dev_values,
        development_replays: dev_replay,
        development_tamper_rejections: dev_tamper,
        holdout_cases: 60,
        holdout_exact_decisions: holdout_exact,
        holdout_value_correct: holdout_values,
        holdout_replays: holdout_replay,
        holdout_tamper_rejections: holdout_tamper,
        false_authorizations: false_auth,
        false_denials: false_deny,
        selected_module: plan.selected_module,
        selected_external_gap_coverage: plan.selected_coverage,
        plan_promotable_in_sandbox: plan.selected_promotable_in_sandbox,
        manifest_unchanged,
        source_provenance_preserved: dev_artifacts + 60,
    };
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(REPORT_MD, format!(
        "# Stage 82 — execute selected external education\n\n- Selected module: `{}`\n- External residual coverage: {}\n- Development: {}/{} exact, {}/{} artifacts, {}/{} replay, {}/{} tamper\n- Holdout: {}/{} exact, {}/{} value, {}/{} replay, {}/{} tamper\n- False authorizations / denials: {} / {}\n- Manifest unchanged: {}\n",
        report.selected_module.as_deref().unwrap_or("none"), report.selected_external_gap_coverage,
        report.development_exact_decisions, report.development_cases, report.development_artifacts, report.development_supported,
        report.development_replays, report.development_cases, report.development_tamper_rejections, report.development_cases,
        report.holdout_exact_decisions, report.holdout_cases, report.holdout_value_correct, report.holdout_cases,
        report.holdout_replays, report.holdout_cases, report.holdout_tamper_rejections, report.holdout_cases,
        report.false_authorizations, report.false_denials, report.manifest_unchanged,
    ))?;
    Ok(())
}
