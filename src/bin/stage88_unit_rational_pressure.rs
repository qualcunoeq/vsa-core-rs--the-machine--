//! Stage 88: pressure-test exact rational unit conversion.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaStatus,
};
use the_machine::source_unit_frontend::{formalize_unit_text, replay_verified, UnitFrontendStatus};

const SOURCE: &str = include_str!("../../docs/sources/openstax_unit_conversion_catalog.txt");
const DOMAIN: &str = "source_catalog_unit_conversion";
const JSON: &str = "docs/stage88_unit_rational_pressure.json";
const MD: &str = "docs/stage88_unit_rational_pressure.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_sha256: String,
    cases: usize,
    supported: usize,
    refused: usize,
    exact: usize,
    value_correct: usize,
    frontend_replay: usize,
    execution_replay: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn amount(index: usize) -> (&'static str, i128, i128) {
    match index % 3 {
        0 => ("1.5", 3, 2),
        1 => ("7/4", 7, 4),
        _ => ("2.25", 9, 4),
    }
}

fn unit_pair(index: usize) -> (&'static str, &'static str, i128) {
    match index % 4 {
        0 => ("meters", "centimeters", 100),
        1 => ("hours", "minutes", 60),
        2 => ("pounds", "ounces", 16),
        _ => ("liters", "milliliters", 1000),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let records = extract_formula_records(SOURCE).map_err(|errors| errors.join("; "))?;
    let mut exact = 0;
    let mut values = 0;
    let mut frontend_replay = 0;
    let mut execution_replay = 0;
    let mut tamper = 0;
    let mut false_auth = 0;
    let mut false_deny = 0;
    for index in 0..180 {
        let (amount_text, numerator, denominator) = amount(index);
        let (source, target, factor) = unit_pair(index);
        let text = format!("Convert {amount_text} {source} to {target}.");
        let frontend = formalize_unit_text(&text, &format!("stage88-supported-{index}"), &records);
        let mut frontend_copy = frontend.clone();
        frontend_copy.replay_hash.push('x');
        let front_ok =
            frontend.status == UnitFrontendStatus::Complete && replay_verified(&frontend);
        let front_tamper = !replay_verified(&frontend_copy);
        let result = evaluate_formula_records(frontend.request.as_ref().unwrap(), DOMAIN, &records);
        let expected_numerator = numerator * factor;
        let expected_value =
            the_machine::probability_pack::Rational::new(expected_numerator, denominator).unwrap();
        let value_ok = result.status == FormulaStatus::Complete
            && result.value.as_ref() == Some(&expected_value);
        let mut result_copy = result.clone();
        result_copy.replay_hash.push('x');
        let execution_ok = result.replay_verified();
        let execution_tamper = !result_copy.replay_verified();
        exact += usize::from(front_ok && value_ok && execution_ok);
        values += usize::from(value_ok);
        frontend_replay += usize::from(replay_verified(&frontend));
        execution_replay += usize::from(execution_ok);
        tamper += usize::from(front_tamper && execution_tamper);
        false_deny += usize::from(!(front_ok && value_ok && execution_ok));
    }
    for index in 0..120 {
        let text = match index % 4 {
            0 => "Convert 0 meters to centimeters.",
            1 => "Convert -2 hours to minutes.",
            2 => "Convert 3 meters to yards.",
            _ => "Convert approximately 3 meters to centimeters.",
        };
        let frontend = formalize_unit_text(text, &format!("stage88-refused-{index}"), &records);
        let mut copy = frontend.clone();
        copy.replay_hash.push('x');
        let safe = if let Some(request) = frontend.request.as_ref() {
            evaluate_formula_records(request, DOMAIN, &records).status != FormulaStatus::Complete
        } else {
            true
        };
        exact += usize::from(safe && replay_verified(&frontend));
        frontend_replay += usize::from(replay_verified(&frontend));
        tamper += usize::from(!replay_verified(&copy));
        false_auth += usize::from(!safe);
    }
    assert_eq!(exact, 300);
    assert_eq!(values, 180);
    assert_eq!(frontend_replay, 300);
    assert_eq!(execution_replay, 180);
    assert_eq!(tamper, 300);
    assert_eq!((false_auth, false_deny), (0, 0));
    let report = Report {
        schema: "stage88-unit-rational-pressure-v1",
        source_sha256: digest(&SOURCE),
        cases: 300,
        supported: 180,
        refused: 120,
        exact,
        value_correct: values,
        frontend_replay,
        execution_replay,
        tamper_rejections: tamper,
        false_authorizations: false_auth,
        false_denials: false_deny,
    };
    fs::write(
        JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(MD, format!(
        "# Stage 88 — exact rational unit pressure\n\n- Cases: {} (supported {}, refused {})\n- Exact decisions: {}/{}\n- Values: {}/{}\n- Frontend replay: {}/{}\n- Execution replay: {}/{}\n- Tamper rejection: {}/{}\n- False authorizations / denials: {} / {}\n- Source SHA-256: `{}`\n",
        report.cases, report.supported, report.refused, report.exact, report.cases, report.value_correct, report.supported, report.frontend_replay, report.cases, report.execution_replay, report.supported, report.tamper_rejections, report.cases, report.false_authorizations, report.false_denials, report.source_sha256,
    ))?;
    Ok(())
}
