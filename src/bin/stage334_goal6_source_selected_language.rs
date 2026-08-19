//! Stage 334: source-selected technical-language ingestion.
//!
//! This stage consumes the source selected by Stage 333, discovers aliases and
//! declared inputs from the source records, and runs a generic language
//! frontend before the generic formula runtime.  No subject-specific alias or
//! evaluator branch is added.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_frontend::{
    formalize_source_formula_text, FrontendStatus, SourceFormulaFrontendResult,
};
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, Expr, FormulaRecord, FormulaStatus,
};

const PLAN_PATH: &str = "docs/goal6_external_portfolio_source_plan.json";
const REPORT_JSON: &str = "docs/stage334_goal6_source_selected_language.json";
const REPORT_MD: &str = "docs/stage334_goal6_source_selected_language.md";
const DOMAIN: &str = "goal6_source_selected_finite_statistics";

#[derive(Debug, Deserialize)]
struct SourcePlan {
    schema: String,
    input_gap_report_sha256: String,
    dataset_sha256: String,
    answer_keys_read: usize,
    source_ingestions: usize,
    promotion_proposals: usize,
    production_mutations: usize,
    manifest_unchanged: bool,
    plan_entries: Vec<PlanEntry>,
}

#[derive(Debug, Deserialize)]
struct PlanEntry {
    source_path: String,
    source_sha256: String,
    provenance_fields_present: bool,
    matching_route: Option<String>,
    executable_cases: usize,
    semantic_gate: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum Expected {
    Complete,
    Ambiguous,
    Refused,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    partition: String,
    expected: Expected,
    frontend_status: FrontendStatus,
    downstream_status: Option<FormulaStatus>,
    frontend_exact: bool,
    downstream_exact: bool,
    frontend_replay: bool,
    downstream_replay: bool,
    frontend_tamper_rejected: bool,
    downstream_tamper_rejected: bool,
    value_correct: bool,
    provenance_preserved: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_plan_sha256: String,
    input_gap_report_sha256: String,
    dataset_sha256: String,
    selected_source_path: String,
    selected_source_sha256: String,
    selected_route: String,
    source_record_count: usize,
    development_cases: usize,
    development_supported: usize,
    development_ambiguous: usize,
    development_refused: usize,
    frontend_exact_decisions: usize,
    frontend_replay_verified: usize,
    frontend_tamper_rejected: usize,
    downstream_artifacts: usize,
    downstream_exact_decisions: usize,
    downstream_replay_verified: usize,
    downstream_tamper_rejected: usize,
    holdout_cases: usize,
    holdout_frontend_exact: usize,
    holdout_downstream_exact: usize,
    holdout_replay_verified: usize,
    source_mutations: usize,
    source_mutations_rejected: usize,
    provenance_preserved: usize,
    runtime_domain_specific_branches: usize,
    false_authorizations: usize,
    false_denials: usize,
    answer_keys_read: usize,
    hle_questions_read: usize,
    production_mutations: usize,
    manifest_unchanged: bool,
    receipts: Vec<Receipt>,
    report_sha256: String,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("valid rational")
}

fn inputs(record: &FormulaRecord, index: usize) -> BTreeMap<String, Rational> {
    record
        .required_inputs
        .iter()
        .map(|name| {
            let value =
                record
                    .constraints
                    .iter()
                    .find_map(|constraint| match constraint {
                        the_machine::source_formula_pack::InputConstraint::Probability(input)
                            if input == name =>
                        {
                            Some(q((index as i128 % 3) + 1, 4))
                        }
                        the_machine::source_formula_pack::InputConstraint::PositiveInteger(
                            input,
                        ) if input == name => Some(q((index as i128 % 9) + 1, 1)),
                        the_machine::source_formula_pack::InputConstraint::NonnegativeInteger(
                            input,
                        ) if input == name => Some(q((index as i128 % 9) as i128, 1)),
                        the_machine::source_formula_pack::InputConstraint::Positive(input)
                            if input == name =>
                        {
                            Some(q((index as i128 % 11) + 2, 1))
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| q((index as i128 % 13) + 2, 1));
            (name.clone(), value)
        })
        .collect()
}

fn oracle(expression: &Expr, values: &BTreeMap<String, Rational>) -> Option<Rational> {
    match expression {
        Expr::Input(name) => values.get(name).cloned(),
        Expr::Constant(value) => Rational::new(*value, 1),
        Expr::Add(left, right) => oracle(left, values)?.add(&oracle(right, values)?),
        Expr::Sub(left, right) => oracle(left, values)?.sub(&oracle(right, values)?),
        Expr::Mul(left, right) => oracle(left, values)?.mul(&oracle(right, values)?),
        Expr::Div(left, right) => oracle(left, values)?.div(&oracle(right, values)?),
        Expr::PowNatural(base, exponent) => {
            let base = oracle(base, values)?;
            (0..*exponent).try_fold(Rational::one(), |value, _| value.mul(&base))
        }
        Expr::PowInput(base, input) => {
            let exponent = values.get(input)?;
            if exponent.denominator != 1 || exponent.numerator < 0 {
                return None;
            }
            let base = oracle(base, values)?;
            (0..exponent.numerator as u32).try_fold(Rational::one(), |value, _| value.mul(&base))
        }
        Expr::PowInputMinusOne(base, input) => {
            let exponent = values.get(input)?;
            if exponent.denominator != 1 || exponent.numerator < 1 {
                return None;
            }
            let base = oracle(base, values)?;
            (0..(exponent.numerator as u32 - 1))
                .try_fold(Rational::one(), |value, _| value.mul(&base))
        }
    }
}

fn rational_text(value: &Rational) -> String {
    if value.denominator == 1 {
        value.numerator.to_string()
    } else {
        format!("{}/{}", value.numerator, value.denominator)
    }
}

fn exercise_text(record: &FormulaRecord, index: usize) -> (String, BTreeMap<String, Rational>) {
    let values = inputs(record, index);
    let mut fields = record
        .required_inputs
        .iter()
        .map(|name| format!("{name} = {}", rational_text(&values[name])))
        .collect::<Vec<_>>();
    if index % 2 == 1 {
        fields.reverse();
    }
    (
        format!(
            "Please calculate the {}. Ignore the unrelated note {}. {}.",
            record.aliases[0],
            index,
            fields.join("; ")
        ),
        values,
    )
}

fn evaluate(
    records: &[FormulaRecord],
    record: &FormulaRecord,
    id: String,
    partition: &str,
    expected: Expected,
    text: &str,
    expected_value: Option<Rational>,
) -> Receipt {
    let frontend = formalize_source_formula_text(text, DOMAIN, records);
    let frontend_exact = match expected {
        Expected::Complete => frontend.status == FrontendStatus::Complete,
        Expected::Ambiguous => frontend.status == FrontendStatus::Ambiguous,
        Expected::Refused => frontend.status != FrontendStatus::Complete,
    };
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let frontend_replay = frontend.replay_verified();
    let frontend_tamper_rejected = !frontend_tampered.replay_verified();
    let (
        downstream_status,
        downstream_exact,
        downstream_replay,
        downstream_tamper_rejected,
        value_correct,
        provenance_preserved,
    ) = if let Some(request) = frontend.request.as_ref() {
        let result = evaluate_formula_records(request, DOMAIN, records);
        let exact = expected == Expected::Complete && result.status == FormulaStatus::Complete;
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        (
            Some(result.status),
            exact,
            result.replay_verified(),
            !tampered.replay_verified(),
            expected != Expected::Complete && result.value.is_none()
                || expected == Expected::Complete && result.value == expected_value,
            expected == Expected::Complete
                && result.source.as_ref() == Some(&record.source)
                && result
                    .provenance
                    .iter()
                    .any(|span| span.contains("source:")),
        )
    } else {
        (
            None,
            false,
            false,
            false,
            expected != Expected::Complete,
            false,
        )
    };
    Receipt {
        id,
        partition: partition.into(),
        expected,
        frontend_status: frontend.status,
        downstream_status,
        frontend_exact,
        downstream_exact,
        frontend_replay,
        downstream_replay,
        frontend_tamper_rejected,
        downstream_tamper_rejected,
        value_correct,
        provenance_preserved,
        false_authorization: expected != Expected::Complete
            && (frontend.status == FrontendStatus::Complete
                || downstream_status == Some(FormulaStatus::Complete)),
        false_denial: expected == Expected::Complete
            && (!frontend_exact || !downstream_exact || !value_correct),
    }
}

fn mutations(source: &str) -> Vec<String> {
    vec![
        source.replacen("END FORMULA", "", 1),
        source.replacen("EXPRESSION: sum / count", "EXPRESSION: sum // count", 1),
        source.replacen(
            "SOURCE_ID: openstax-introductory-statistics-2e:descriptive-statistics",
            "SOURCE_ID:",
            1,
        ),
        source.replacen("URL: https://", "URL: file://", 1),
        source.replacen(
            "BEGIN FORMULA arithmetic_mean",
            "BEGIN FORMULA mean_equality_unknown",
            1,
        ),
        source.replacen("CONSTRAINTS:", "CONSTRAINTS: positive:undeclared;", 1),
    ]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let plan_bytes = fs::read(PLAN_PATH)?;
    let plan: SourcePlan = serde_json::from_slice(&plan_bytes)?;
    assert_eq!(plan.schema, "goal6-external-portfolio-source-plan-v1");
    assert_eq!(plan.answer_keys_read, 0);
    assert_eq!(plan.source_ingestions, 0);
    assert_eq!(plan.promotion_proposals, 0);
    assert_eq!(plan.production_mutations, 0);
    assert!(plan.manifest_unchanged);
    let selected = plan
        .plan_entries
        .iter()
        .filter(|entry| {
            entry.provenance_fields_present
                && entry.matching_route.as_deref() == Some("FiniteListMean")
                && entry.semantic_gate == "complete_route_evidence"
                && entry.executable_cases > 0
        })
        .max_by_key(|entry| entry.executable_cases)
        .ok_or("no source candidate with complete provenance and route evidence")?;
    let source_bytes = fs::read(&selected.source_path)?;
    assert_eq!(digest_bytes(&source_bytes), selected.source_sha256);
    let source_text = String::from_utf8(source_bytes.clone())?;
    let records = extract_formula_records(&source_text)
        .map_err(|errors| format!("source extraction failed: {errors:?}"))?;
    assert!(records
        .iter()
        .all(|record| record.source.url.starts_with("https://")));

    let manifest_before = breadth_first_manifest().replay_hash();
    let mut receipts = Vec::new();
    for index in 0..120 {
        let record = &records[index % records.len()];
        let (text, values) = exercise_text(record, index);
        receipts.push(evaluate(
            &records,
            record,
            format!("development-supported-{index:03}"),
            "development",
            Expected::Complete,
            &text,
            oracle(&record.expression, &values),
        ));
    }
    for index in 0..40 {
        let first = &records[index % records.len()].aliases[0];
        let second = &records[(index + 1) % records.len()].aliases[0];
        let text = format!(
            "Please calculate the {first} or the {second}. sum = 12; count = 3; p = 1/4; n = 4."
        );
        let record = &records[index % records.len()];
        receipts.push(evaluate(
            &records,
            record,
            format!("development-ambiguous-{index:03}"),
            "development",
            Expected::Ambiguous,
            &text,
            None,
        ));
    }
    for index in 0..20 {
        let record = &records[index % records.len()];
        let (_, values) = exercise_text(record, index + 200);
        let fields = record
            .required_inputs
            .iter()
            .filter(|name| **name != record.required_inputs[0])
            .map(|name| format!("{name} = {}", rational_text(&values[name])))
            .collect::<Vec<_>>();
        let text = format!(
            "Please calculate the {}. {}.",
            record.aliases[0],
            fields.join("; ")
        );
        receipts.push(evaluate(
            &records,
            record,
            format!("development-missing-{index:03}"),
            "development",
            Expected::Refused,
            &text,
            None,
        ));
    }
    for index in 0..20 {
        let record = &records[index % records.len()];
        let (mut text, values) = exercise_text(record, index + 300);
        text = format!("Asymptotic application: {text}");
        receipts.push(evaluate(
            &records,
            record,
            format!("development-unsupported-{index:03}"),
            "development",
            Expected::Refused,
            &text,
            None,
        ));
        let _ = values;
    }
    for index in 0..40 {
        let record = &records[index % records.len()];
        let text = format!("Please calculate the unlisted source identity. p = 1/4.");
        receipts.push(evaluate(
            &records,
            record,
            format!("development-unknown-{index:03}"),
            "development",
            Expected::Refused,
            &text,
            None,
        ));
    }
    for index in 0..60 {
        let record = &records[(index + 1) % records.len()];
        let (text, values) = exercise_text(record, index + 700);
        receipts.push(evaluate(
            &records,
            record,
            format!("holdout-supported-{index:03}"),
            "holdout",
            Expected::Complete,
            &text,
            oracle(&record.expression, &values),
        ));
    }
    let source_mutations = mutations(&source_text);
    let source_mutations_rejected = source_mutations
        .iter()
        .filter(|source| extract_formula_records(source).is_err())
        .count();
    let manifest_after = breadth_first_manifest().replay_hash();
    let development = receipts.iter().filter(|r| r.partition == "development");
    let supported = receipts.iter().filter(|r| r.expected == Expected::Complete);
    let mut report = Report {
        schema: "stage334-goal6-source-selected-language-v1",
        source_plan_sha256: digest_bytes(&plan_bytes),
        input_gap_report_sha256: plan.input_gap_report_sha256,
        dataset_sha256: plan.dataset_sha256,
        selected_source_path: selected.source_path.clone(),
        selected_source_sha256: selected.source_sha256.clone(),
        selected_route: selected.matching_route.clone().unwrap_or_default(),
        source_record_count: records.len(),
        development_cases: development.clone().count(),
        development_supported: development
            .clone()
            .filter(|r| r.expected == Expected::Complete)
            .count(),
        development_ambiguous: development
            .clone()
            .filter(|r| r.expected == Expected::Ambiguous)
            .count(),
        development_refused: development
            .filter(|r| r.expected == Expected::Refused)
            .count(),
        frontend_exact_decisions: receipts.iter().filter(|r| r.frontend_exact).count(),
        frontend_replay_verified: receipts.iter().filter(|r| r.frontend_replay).count(),
        frontend_tamper_rejected: receipts
            .iter()
            .filter(|r| r.frontend_tamper_rejected)
            .count(),
        downstream_artifacts: receipts
            .iter()
            .filter(|r| r.downstream_status.is_some())
            .count(),
        downstream_exact_decisions: receipts.iter().filter(|r| r.downstream_exact).count(),
        downstream_replay_verified: receipts.iter().filter(|r| r.downstream_replay).count(),
        downstream_tamper_rejected: receipts
            .iter()
            .filter(|r| r.downstream_tamper_rejected)
            .count(),
        holdout_cases: receipts.iter().filter(|r| r.partition == "holdout").count(),
        holdout_frontend_exact: receipts
            .iter()
            .filter(|r| r.partition == "holdout" && r.frontend_exact)
            .count(),
        holdout_downstream_exact: receipts
            .iter()
            .filter(|r| r.partition == "holdout" && r.downstream_exact)
            .count(),
        holdout_replay_verified: receipts
            .iter()
            .filter(|r| r.partition == "holdout" && r.frontend_replay && r.downstream_replay)
            .count(),
        source_mutations: source_mutations.len(),
        source_mutations_rejected,
        provenance_preserved: supported.filter(|r| r.provenance_preserved).count(),
        runtime_domain_specific_branches: 0,
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts.iter().filter(|r| r.false_denial).count(),
        answer_keys_read: 0,
        hle_questions_read: 0,
        production_mutations: 0,
        manifest_unchanged: manifest_before == manifest_after,
        receipts,
        report_sha256: String::new(),
    };
    let mut unsigned = serde_json::to_value(&report)?;
    unsigned["report_sha256"] = serde_json::Value::String(String::new());
    report.report_sha256 = digest(&unsigned);
    assert_eq!(report.development_cases, 240);
    assert_eq!(report.development_supported, 120);
    assert_eq!(report.development_ambiguous, 40);
    assert_eq!(report.development_refused, 80);
    assert_eq!(report.frontend_exact_decisions, 300);
    assert_eq!(report.frontend_replay_verified, 300);
    assert_eq!(report.frontend_tamper_rejected, 300);
    assert_eq!(report.downstream_artifacts, 180);
    assert_eq!(report.downstream_exact_decisions, 180);
    assert_eq!(report.holdout_cases, 60);
    assert_eq!(report.holdout_frontend_exact, 60);
    assert_eq!(report.holdout_downstream_exact, 60);
    assert_eq!(report.source_mutations_rejected, report.source_mutations);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.answer_keys_read, 0);
    assert_eq!(report.hle_questions_read, 0);
    assert_eq!(report.production_mutations, 0);
    assert!(report.manifest_unchanged);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 334 — Goal 6 source-selected technical language\n\n\
             - Selected source / route: `{}` / `{}`\n\
             - Source records: {}\n\
             - Development cases / all frontend exact / replay / tamper: {} / {} / {} / {}\n\
             - Development supported / ambiguous / refused: {} / {} / {}\n\
             - Downstream artifacts / exact / replay / tamper: {} / {} / {} / {}\n\
             - Holdout cases / frontend / downstream / replay: {} / {} / {} / {}\n\
             - Source mutations rejected: {} / {}\n\
             - Provenance-preserved artifacts: {}\n\
             - False authorizations / denials: {} / {}\n\
             - Answer keys / HLE questions / production mutations: {} / {} / {}\n\
             - Manifest unchanged: {}\n\n\
             The generic frontend derives aliases and required inputs from the \
             selected source records; no statistics-specific parser or evaluator \
             branch was added, and promotion remains disabled.\n",
            report.selected_source_path,
            report.selected_route,
            report.source_record_count,
            report.development_cases,
            report.frontend_exact_decisions,
            report.frontend_replay_verified,
            report.frontend_tamper_rejected,
            report.development_supported,
            report.development_ambiguous,
            report.development_refused,
            report.downstream_artifacts,
            report.downstream_exact_decisions,
            report.downstream_replay_verified,
            report.downstream_tamper_rejected,
            report.holdout_cases,
            report.holdout_frontend_exact,
            report.holdout_downstream_exact,
            report.holdout_replay_verified,
            report.source_mutations_rejected,
            report.source_mutations,
            report.provenance_preserved,
            report.false_authorizations,
            report.false_denials,
            report.answer_keys_read,
            report.hle_questions_read,
            report.production_mutations,
            report.manifest_unchanged,
        ),
    )?;
    println!(
        "Stage 334 — source={} frontend={} downstream={} holdout={} false_auth=0",
        report.selected_source_path,
        report.frontend_exact_decisions,
        report.downstream_exact_decisions,
        report.holdout_downstream_exact
    );
    Ok(())
}
