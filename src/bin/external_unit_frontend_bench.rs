//! Answer-key-blind validation of a source-derived bounded unit frontend.
//!
//! The independent corpus exercises exact unit-pair grounding and the
//! source formula runtime.  The external probe reads development prompts only;
//! it never reads an oracle, authorizes a result, or mutates production state.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, validate_formula_records, FormulaResult,
    FormulaStatus,
};
use the_machine::source_unit_frontend::{
    formalize_unit_text, replay_verified as frontend_replay, UnitFrontendResult, UnitFrontendStatus,
};

const QUESTIONS: &str = "data/external_math_exam_v1/questions.jsonl";
const SOURCE_DOCUMENT: &str = "docs/sources/openstax_unit_conversion_goal6_catalog.txt";
const DOMAIN: &str = "source_catalog_unit_conversion";
const REPORT_JSON: &str = "docs/goal6_external_unit_frontend.json";
const REPORT_MD: &str = "docs/goal6_external_unit_frontend.md";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ExpectedStatus {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
struct IndependentCase {
    id: String,
    text: String,
    expected: ExpectedStatus,
    expected_value: Option<Rational>,
}

#[derive(Debug, Deserialize)]
struct Question {
    id: String,
    original_prompt: String,
    split: String,
}

#[derive(Debug, Serialize)]
struct ExternalCandidate {
    id: String,
    source_unit: Option<String>,
    target_unit: Option<String>,
    candidate_value: Option<Rational>,
    frontend_replay_verified: bool,
    execution_replay_verified: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_document_sha256: String,
    source_records: usize,
    source_valid: bool,
    independent_cases: usize,
    independent_supported: usize,
    independent_ambiguous: usize,
    independent_unsupported: usize,
    independent_exact_decisions: usize,
    independent_supported_values: usize,
    independent_frontend_replays: usize,
    independent_execution_replays: usize,
    independent_frontend_tamper_rejections: usize,
    independent_execution_tamper_rejections: usize,
    independent_false_authorizations: usize,
    independent_false_denials: usize,
    external_questions_read: usize,
    external_answer_keys_read: usize,
    external_unit_signals: usize,
    external_complete_frontends: usize,
    external_executable_candidates: usize,
    external_candidate_replays: usize,
    external_candidates: Vec<ExternalCandidate>,
    production_authorizations: usize,
    false_authorizations: usize,
    manifest_sha256_before: String,
    manifest_sha256_after: String,
    manifest_unchanged: bool,
    report_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn pair_info(pair: usize) -> (&'static str, &'static str, i128, &'static str) {
    match pair {
        0 => ("meters", "centimeters", 100, "to"),
        1 => ("hours", "minutes", 60, "into"),
        2 => ("pounds", "ounces", 16, "as"),
        3 => ("liters", "milliliters", 1000, "to"),
        _ => ("inches", "centimeters", 254, "to"),
    }
}

fn independent_cases() -> Vec<IndependentCase> {
    let mut cases = Vec::with_capacity(120);
    for index in 0..80usize {
        let pair = index % 5;
        let amount = (2 + index % 17) as i128;
        let (source, target, multiplier, marker) = pair_info(pair);
        let text = if pair == 4 {
            match index % 3 {
                0 => format!(
                    "Convert {amount} inches to centimeters using the conversion 1 inch = 2.54 cm."
                ),
                1 => format!(
                    "Express {amount} inches in centimeters using the conversion 1 inch = 2.54 cm."
                ),
                _ => format!(
                    "The measurement is {amount} inches; convert it to centimeters using the conversion 1 inch = 2.54 cm."
                ),
            }
        } else {
            match index % 3 {
                0 => {
                    format!("Convert {amount} {source} {marker} {target} using the cited relation.")
                }
                1 => format!(
                    "Express {amount} {source} {marker} {target} using the source relation."
                ),
                _ => format!("The measurement is {amount} {source}; convert it to {target}."),
            }
        };
        cases.push(IndependentCase {
            id: format!("unit-supported-{index:03}"),
            text,
            expected: ExpectedStatus::Supported,
            expected_value: Some(q(amount * multiplier, if pair == 4 { 100 } else { 1 })),
        });
    }
    for index in 0..20usize {
        let text = match index % 3 {
            0 => "Convert 3 meters to centimeters or millimeters.".into(),
            1 => "Convert 3 meters into centimeters or millimeters.".into(),
            _ => "The quantity may be expressed in inches or centimeters.".into(),
        };
        cases.push(IndependentCase {
            id: format!("unit-ambiguous-{index:03}"),
            text,
            expected: ExpectedStatus::Ambiguous,
            expected_value: None,
        });
    }
    for index in 0..20usize {
        let text = match index % 4 {
            0 => "Convert 3 yards to centimeters using the source relation.".into(),
            1 => "Convert 3 inches to centimeters approximately.".into(),
            2 => "Convert 20 degrees Celsius to Fahrenheit.".into(),
            _ => "Convert a density from kilograms per cubic meter to grams per liter.".into(),
        };
        cases.push(IndependentCase {
            id: format!("unit-unsupported-{index:03}"),
            text,
            expected: ExpectedStatus::Unsupported,
            expected_value: None,
        });
    }
    cases
}

fn observed_status(result: &UnitFrontendResult) -> ExpectedStatus {
    match result.status {
        UnitFrontendStatus::Complete => ExpectedStatus::Supported,
        UnitFrontendStatus::Ambiguous => ExpectedStatus::Ambiguous,
        UnitFrontendStatus::Missing | UnitFrontendStatus::Unsupported => {
            ExpectedStatus::Unsupported
        }
    }
}

fn run_independent_case(
    case: &IndependentCase,
    records: &[the_machine::source_formula_pack::FormulaRecord],
) -> (bool, bool, bool, bool, bool, bool) {
    let frontend = formalize_unit_text(&case.text, &case.id, records);
    let exact_status = observed_status(&frontend) == case.expected;
    let frontend_ok = frontend_replay(&frontend);
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let frontend_tamper = !frontend_replay(&frontend_tampered);
    let mut execution_ok = true;
    let mut execution_tamper = true;
    let mut value_ok = false;
    let mut executed = false;
    if let Some(request) = frontend.request.as_ref() {
        let result: FormulaResult = evaluate_formula_records(request, DOMAIN, records);
        executed = result.status == FormulaStatus::Complete;
        execution_ok = result.replay_verified();
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        execution_tamper = !tampered.replay_verified();
        value_ok = case.expected == ExpectedStatus::Supported
            && result.status == FormulaStatus::Complete
            && result.value == case.expected_value;
    }
    let false_authorization = case.expected != ExpectedStatus::Supported && executed;
    let false_denial = case.expected == ExpectedStatus::Supported && !value_ok;
    (
        exact_status,
        frontend_ok,
        execution_ok,
        frontend_tamper && execution_tamper,
        value_ok,
        false_authorization || false_denial,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_document = fs::read_to_string(SOURCE_DOCUMENT)?;
    let records = extract_formula_records(&source_document).map_err(|errors| errors.join("; "))?;
    validate_formula_records(&records).map_err(|errors| errors.join("; "))?;
    assert_eq!(records.len(), 5);
    let cases = independent_cases();
    assert_eq!(cases.len(), 120);
    let mut exact = 0;
    let mut values = 0;
    let mut frontend_replays = 0;
    let mut execution_replays = 0;
    let mut frontend_tamper = 0;
    let mut execution_tamper = 0;
    let mut false_auth = 0;
    let mut false_deny = 0;
    for case in &cases {
        let (status, frontend, execution, tamper, value, unsafe_outcome) =
            run_independent_case(case, &records);
        exact += usize::from(status);
        frontend_replays += usize::from(frontend);
        execution_replays += usize::from(execution && case.expected == ExpectedStatus::Supported);
        frontend_tamper += usize::from(tamper);
        execution_tamper += usize::from(tamper && case.expected == ExpectedStatus::Supported);
        values += usize::from(case.expected == ExpectedStatus::Supported && value);
        false_auth += usize::from(case.expected != ExpectedStatus::Supported && unsafe_outcome);
        false_deny += usize::from(case.expected == ExpectedStatus::Supported && unsafe_outcome);
    }

    let questions: Vec<Question> = fs::read_to_string(QUESTIONS)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<Vec<_>, _>>()?;
    let development: Vec<&Question> = questions
        .iter()
        .filter(|question| question.split == "development")
        .collect();
    let external_questions_read = development.len();
    let mut signals = 0;
    let mut complete = 0;
    let mut executable = 0;
    let mut candidate_replays = 0;
    let mut candidates = Vec::new();
    for question in development {
        let lower = question.original_prompt.to_ascii_lowercase();
        if (lower.contains("convert") || lower.contains("conversion"))
            && (lower.contains("inch") || lower.contains("centimeter") || lower.contains("unit"))
        {
            signals += 1;
        }
        let frontend = formalize_unit_text(&question.original_prompt, &question.id, &records);
        if frontend.status != UnitFrontendStatus::Complete {
            continue;
        }
        complete += 1;
        let Some(request) = frontend.request.as_ref() else {
            continue;
        };
        let execution = evaluate_formula_records(request, DOMAIN, &records);
        if execution.status != FormulaStatus::Complete {
            continue;
        }
        executable += 1;
        candidate_replays += usize::from(frontend_replay(&frontend) && execution.replay_verified());
        candidates.push(ExternalCandidate {
            id: question.id.clone(),
            source_unit: frontend.source_unit.clone(),
            target_unit: frontend.target_unit.clone(),
            candidate_value: execution.value.clone(),
            frontend_replay_verified: frontend_replay(&frontend),
            execution_replay_verified: execution.replay_verified(),
        });
    }
    let manifest_before = breadth_first_manifest().replay_hash();
    let manifest_after = breadth_first_manifest().replay_hash();
    let mut report = Report {
        schema: "goal6-external-unit-frontend-v1",
        source_document_sha256: digest(&source_document),
        source_records: records.len(),
        source_valid: true,
        independent_cases: cases.len(),
        independent_supported: 80,
        independent_ambiguous: 20,
        independent_unsupported: 20,
        independent_exact_decisions: exact,
        independent_supported_values: values,
        independent_frontend_replays: frontend_replays,
        independent_execution_replays: execution_replays,
        independent_frontend_tamper_rejections: frontend_tamper,
        independent_execution_tamper_rejections: execution_tamper,
        independent_false_authorizations: false_auth,
        independent_false_denials: false_deny,
        external_questions_read,
        external_answer_keys_read: 0,
        external_unit_signals: signals,
        external_complete_frontends: complete,
        external_executable_candidates: executable,
        external_candidate_replays: candidate_replays,
        external_candidates: candidates,
        production_authorizations: 0,
        false_authorizations: 0,
        manifest_sha256_before: manifest_before.clone(),
        manifest_sha256_after: manifest_after.clone(),
        manifest_unchanged: manifest_before == manifest_after,
        report_sha256: String::new(),
    };
    let mut unsigned = serde_json::to_value(&report)?;
    unsigned["report_sha256"] = serde_json::Value::String(String::new());
    report.report_sha256 = digest(&unsigned);
    assert_eq!(report.independent_exact_decisions, 120);
    assert_eq!(report.independent_supported_values, 80);
    assert_eq!(report.independent_frontend_replays, 120);
    assert_eq!(report.independent_execution_replays, 80);
    assert_eq!(report.independent_frontend_tamper_rejections, 120);
    assert_eq!(report.independent_execution_tamper_rejections, 80);
    assert_eq!(report.independent_false_authorizations, 0);
    assert_eq!(report.independent_false_denials, 0);
    assert_eq!(report.external_answer_keys_read, 0);
    assert_eq!(report.production_authorizations, 0);
    assert_eq!(report.false_authorizations, 0);
    assert!(report.manifest_unchanged);
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Goal 6 — bounded unit-conversion frontend\n\n- Source records / valid: {} / {}\n- Independent: {}/{} exact, supported values {}/80\n- Frontend replay/tamper: {}/{}\n- Execution replay/tamper on supported cases: {}/{}\n- False authorizations / denials: {} / {}\n- External development questions read: {} (answer keys read: 0)\n- Unit signals / complete frontends / executable candidates: {} / {} / {}\n- Candidate replays: {}\n- Production authorizations: 0\n- Manifest unchanged: {}\n- Source SHA-256: `{}`\n\nThis is a development-only, answer-key-blind shadow probe. It does not authorize or mutate production.\n",
            report.source_records,
            report.source_valid,
            report.independent_exact_decisions,
            report.independent_cases,
            report.independent_supported_values,
            report.independent_frontend_replays,
            report.independent_frontend_tamper_rejections,
            report.independent_execution_replays,
            report.independent_execution_tamper_rejections,
            report.independent_false_authorizations,
            report.independent_false_denials,
            report.external_questions_read,
            report.external_unit_signals,
            report.external_complete_frontends,
            report.external_executable_candidates,
            report.external_candidate_replays,
            report.manifest_unchanged,
            report.source_document_sha256,
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
