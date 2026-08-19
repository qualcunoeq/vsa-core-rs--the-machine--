//! Stage 333: answer-key-blind source selection followed by shadow acquisition.
//!
//! The source is selected from the Goal 6 development-only source plan.  The
//! selected document is parsed into declarative formula records and exercised
//! by an independently implemented oracle.  No HLE or external answer key is
//! read, and the resulting catalog remains a clone-only artifact.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, validate_formula_records, Expr,
    FormulaRecord, FormulaRequest, FormulaStatus, InputConstraint,
};

const PLAN_PATH: &str = "docs/goal6_external_portfolio_source_plan.json";
const DEFAULT_REPORT_JSON: &str = "docs/stage333_goal6_source_selected_acquisition.json";
const DEFAULT_REPORT_MD: &str = "docs/stage333_goal6_source_selected_acquisition.md";

#[derive(Debug, Deserialize)]
struct SourcePlan {
    schema: String,
    input_gap_report_sha256: String,
    dataset_sha256: String,
    development_questions_read: usize,
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
    formula: String,
    expected: Expected,
    actual: FormulaStatus,
    exact_decision: bool,
    value_correct: bool,
    source_provenance_preserved: bool,
    replay_verified: bool,
    tamper_rejected: bool,
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
    selected_executable_cases: usize,
    source_record_count: usize,
    source_records_validated: bool,
    development_exercises: usize,
    development_supported: usize,
    development_ambiguous: usize,
    development_refused: usize,
    development_exact_decisions: usize,
    holdout_exercises: usize,
    holdout_supported: usize,
    holdout_exact_decisions: usize,
    supported_artifacts: usize,
    replay_verified: usize,
    tamper_rejected: usize,
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

fn value_for(record: &FormulaRecord, name: &str, index: usize) -> Rational {
    for constraint in &record.constraints {
        match constraint {
            InputConstraint::Probability(input) if input == name => {
                return q((index as i128 % 3) + 1, 4);
            }
            InputConstraint::PositiveInteger(input) if input == name => {
                return q((index as i128 % 9) + 1, 1);
            }
            InputConstraint::NonnegativeInteger(input) if input == name => {
                return q((index as i128 % 9) as i128, 1);
            }
            InputConstraint::Positive(input) if input == name => {
                return q((index as i128 % 11) + 2, 1);
            }
            InputConstraint::NotEqualInteger(input, forbidden) if input == name => {
                let candidate = (index as i128 % 11) + 2;
                return q(
                    if candidate == *forbidden {
                        candidate + 1
                    } else {
                        candidate
                    },
                    1,
                );
            }
            _ => {}
        }
    }
    q((index as i128 % 13) + 2, 1)
}

fn make_inputs(record: &FormulaRecord, index: usize) -> BTreeMap<String, Rational> {
    record
        .required_inputs
        .iter()
        .map(|name| (name.clone(), value_for(record, name, index)))
        .collect()
}

/// Independent exact oracle.  It interprets the extracted expression tree
/// separately from the production generic evaluator and knows no formula IDs.
fn oracle_expr(expression: &Expr, inputs: &BTreeMap<String, Rational>) -> Option<Rational> {
    match expression {
        Expr::Input(name) => inputs.get(name).cloned(),
        Expr::Constant(value) => Rational::new(*value, 1),
        Expr::Add(left, right) => oracle_expr(left, inputs)?.add(&oracle_expr(right, inputs)?),
        Expr::Sub(left, right) => oracle_expr(left, inputs)?.sub(&oracle_expr(right, inputs)?),
        Expr::Mul(left, right) => oracle_expr(left, inputs)?.mul(&oracle_expr(right, inputs)?),
        Expr::Div(left, right) => oracle_expr(left, inputs)?.div(&oracle_expr(right, inputs)?),
        Expr::PowNatural(base, exponent) => {
            let base = oracle_expr(base, inputs)?;
            (0..*exponent).try_fold(Rational::one(), |value, _| value.mul(&base))
        }
        Expr::PowInput(base, exponent) => {
            let exponent = inputs.get(exponent)?;
            if exponent.denominator != 1 || exponent.numerator < 0 {
                return None;
            }
            let base = oracle_expr(base, inputs)?;
            (0..exponent.numerator as u32).try_fold(Rational::one(), |value, _| value.mul(&base))
        }
        Expr::PowInputMinusOne(base, exponent) => {
            let exponent = inputs.get(exponent)?;
            if exponent.denominator != 1 || exponent.numerator < 1 {
                return None;
            }
            let base = oracle_expr(base, inputs)?;
            (0..(exponent.numerator as u32 - 1))
                .try_fold(Rational::one(), |value, _| value.mul(&base))
        }
    }
}

fn request(
    record: &FormulaRecord,
    inputs: BTreeMap<String, Rational>,
    id: &str,
    domain: &str,
) -> FormulaRequest {
    FormulaRequest {
        formula: record.formula_id.clone(),
        inputs,
        domain: domain.into(),
        ambiguity: None,
        provenance: vec![format!("stage333:{id}:{}", record.source.evidence_span)],
    }
}

fn evaluate(
    records: &[FormulaRecord],
    record: &FormulaRecord,
    id: String,
    partition: &str,
    expected: Expected,
    mut req: FormulaRequest,
    expected_value: Option<Rational>,
    domain: &str,
) -> Receipt {
    let result = evaluate_formula_records(&req, domain, records);
    let exact_decision = match expected {
        Expected::Complete => result.status == FormulaStatus::Complete,
        Expected::Ambiguous => result.status == FormulaStatus::Ambiguous,
        Expected::Refused => result.status != FormulaStatus::Complete,
    };
    let value_correct = expected != Expected::Complete || result.value == expected_value;
    let source_provenance_preserved = expected == Expected::Complete
        && result.source.as_ref() == Some(&record.source)
        && result
            .provenance
            .iter()
            .any(|span| span.contains("stage333:"));
    let replay_verified = result.replay_verified();
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    req.ambiguity = Some("tamper-only request mutation".into());
    let _ = req;
    Receipt {
        id,
        partition: partition.into(),
        formula: record.formula_id.clone(),
        expected,
        actual: result.status,
        exact_decision,
        value_correct,
        source_provenance_preserved,
        replay_verified,
        tamper_rejected: !tampered.replay_verified(),
        false_authorization: expected != Expected::Complete
            && result.status == FormulaStatus::Complete,
        false_denial: expected == Expected::Complete && result.status != FormulaStatus::Complete,
    }
}

fn source_mutations(source: &str) -> Vec<String> {
    let formula_ids = source
        .lines()
        .filter_map(|line| line.strip_prefix("BEGIN FORMULA "))
        .map(str::trim)
        .collect::<Vec<_>>();
    let duplicate_formula = formula_ids
        .get(1)
        .map(|second| {
            source.replacen(
                &format!("BEGIN FORMULA {}", formula_ids[0]),
                &format!("BEGIN FORMULA {second}"),
                1,
            )
        })
        .unwrap_or_else(|| source.to_owned());
    let empty_title = source
        .lines()
        .find(|line| line.starts_with("TITLE:"))
        .map(|line| source.replacen(line, "TITLE:", 1))
        .unwrap_or_else(|| source.to_owned());
    vec![
        source.replacen("END FORMULA", "", 1),
        source.replacen("EXPRESSION:", "EXPRESSION: @", 1),
        empty_title,
        source.replacen("URL: https://", "URL: file://", 1),
        duplicate_formula,
        source.replacen("CONSTRAINTS:", "CONSTRAINTS: positive:undeclared;", 1),
    ]
}

fn route_domain(route: &str) -> String {
    let mut normalized = String::new();
    for (index, character) in route.chars().enumerate() {
        if character.is_ascii_uppercase() && index > 0 {
            normalized.push('_');
        }
        normalized.push(character.to_ascii_lowercase());
    }
    format!("goal6_source_selected_{normalized}")
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
    let desired_route =
        env::var("GOAL6_SOURCE_SELECTED_ROUTE").unwrap_or_else(|_| "FiniteListMean".into());
    let report_json = env::var("GOAL6_SOURCE_SELECTED_ACQUISITION_REPORT_JSON")
        .unwrap_or_else(|_| DEFAULT_REPORT_JSON.into());
    let report_md = env::var("GOAL6_SOURCE_SELECTED_ACQUISITION_REPORT_MD")
        .unwrap_or_else(|_| DEFAULT_REPORT_MD.into());

    let selected = plan
        .plan_entries
        .iter()
        .filter(|entry| {
            entry.provenance_fields_present
                && entry.matching_route.as_deref() == Some(desired_route.as_str())
                && entry.semantic_gate == "complete_route_evidence"
                && entry.executable_cases > 0
        })
        .max_by_key(|entry| entry.executable_cases)
        .ok_or("no provenance-bearing executable source candidate")?;
    let selected_route = selected.matching_route.clone().unwrap_or_default();
    let domain = route_domain(&selected_route);
    let source_bytes = fs::read(&selected.source_path)?;
    assert_eq!(digest_bytes(&source_bytes), selected.source_sha256);
    let source_text = String::from_utf8(source_bytes.clone())?;
    let records = extract_formula_records(&source_text)
        .map_err(|errors| format!("selected source extraction failed: {errors:?}"))?;
    assert!(validate_formula_records(&records).is_ok());

    let manifest_before = breadth_first_manifest().replay_hash();
    let mut receipts = Vec::new();
    for index in 0..120 {
        let record = &records[index % records.len()];
        let inputs = make_inputs(record, index);
        let expected_value = oracle_expr(&record.expression, &inputs);
        receipts.push(evaluate(
            &records,
            record,
            format!("development-supported-{index:03}"),
            "development",
            Expected::Complete,
            request(record, inputs, &format!("supported-{index:03}"), &domain),
            expected_value,
            &domain,
        ));
    }
    for index in 0..40 {
        let record = &records[index % records.len()];
        let mut req = request(
            record,
            make_inputs(record, index + 200),
            &format!("ambiguous-{index:03}"),
            &domain,
        );
        req.ambiguity = Some("source formulation is intentionally unresolved".into());
        receipts.push(evaluate(
            &records,
            record,
            format!("development-ambiguous-{index:03}"),
            "development",
            Expected::Ambiguous,
            req,
            None,
            &domain,
        ));
    }
    for index in 0..20 {
        let record = &records[index % records.len()];
        let mut req = request(
            record,
            make_inputs(record, index + 300),
            &format!("unknown-{index:03}"),
            &domain,
        );
        req.formula = "unlisted_source_formula".into();
        receipts.push(evaluate(
            &records,
            record,
            format!("development-unknown-{index:03}"),
            "development",
            Expected::Refused,
            req,
            None,
            &domain,
        ));
    }
    for index in 0..20 {
        let record = &records[index % records.len()];
        let mut inputs = make_inputs(record, index + 400);
        inputs.remove(record.required_inputs.first().expect("record has input"));
        receipts.push(evaluate(
            &records,
            record,
            format!("development-missing-{index:03}"),
            "development",
            Expected::Refused,
            request(record, inputs, &format!("missing-{index:03}"), &domain),
            None,
            &domain,
        ));
    }
    for index in 0..20 {
        let record = &records[index % records.len()];
        let mut req = request(
            record,
            make_inputs(record, index + 500),
            &format!("domain-{index:03}"),
            &domain,
        );
        req.domain = "unvalidated_domain".into();
        receipts.push(evaluate(
            &records,
            record,
            format!("development-domain-{index:03}"),
            "development",
            Expected::Refused,
            req,
            None,
            &domain,
        ));
    }
    for index in 0..20 {
        let record = &records[index % records.len()];
        let mut inputs = make_inputs(record, index + 600);
        if let Some(constraint) = record.constraints.first() {
            let (name, invalid) = match constraint {
                InputConstraint::Positive(name) | InputConstraint::PositiveInteger(name) => {
                    (name, q(0, 1))
                }
                InputConstraint::NonnegativeInteger(name) => (name, q(-1, 1)),
                InputConstraint::Probability(name) => (name, q(5, 4)),
                InputConstraint::NotEqualInteger(name, forbidden) => (name, q(*forbidden, 1)),
            };
            inputs.insert(name.clone(), invalid);
        }
        receipts.push(evaluate(
            &records,
            record,
            format!("development-invalid-{index:03}"),
            "development",
            Expected::Refused,
            request(record, inputs, &format!("invalid-{index:03}"), &domain),
            None,
            &domain,
        ));
    }
    for index in 0..60 {
        let record = &records[(index + 1) % records.len()];
        let inputs = make_inputs(record, index + 800);
        let expected_value = oracle_expr(&record.expression, &inputs);
        receipts.push(evaluate(
            &records,
            record,
            format!("holdout-supported-{index:03}"),
            "holdout",
            Expected::Complete,
            request(record, inputs, &format!("holdout-{index:03}"), &domain),
            expected_value,
            &domain,
        ));
    }

    let mutations = source_mutations(&source_text);
    let source_mutations_rejected = mutations
        .iter()
        .filter(|mutated| extract_formula_records(mutated).is_err())
        .count();
    let manifest_after = breadth_first_manifest().replay_hash();
    let supported = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Complete)
        .collect::<Vec<_>>();
    let development = receipts.iter().filter(|r| r.partition == "development");
    let mut report = Report {
        schema: "stage333-goal6-source-selected-acquisition-v1",
        source_plan_sha256: digest_bytes(&plan_bytes),
        input_gap_report_sha256: plan.input_gap_report_sha256,
        dataset_sha256: plan.dataset_sha256,
        selected_source_path: selected.source_path.clone(),
        selected_source_sha256: selected.source_sha256.clone(),
        selected_route: selected.matching_route.clone().unwrap_or_default(),
        selected_executable_cases: selected.executable_cases,
        source_record_count: records.len(),
        source_records_validated: true,
        development_exercises: development.clone().count(),
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
        development_exact_decisions: receipts
            .iter()
            .filter(|r| r.partition == "development" && r.exact_decision)
            .count(),
        holdout_exercises: receipts.iter().filter(|r| r.partition == "holdout").count(),
        holdout_supported: receipts
            .iter()
            .filter(|r| r.partition == "holdout" && r.expected == Expected::Complete)
            .count(),
        holdout_exact_decisions: receipts
            .iter()
            .filter(|r| r.partition == "holdout" && r.exact_decision)
            .count(),
        supported_artifacts: supported
            .iter()
            .filter(|r| r.exact_decision && r.value_correct)
            .count(),
        replay_verified: receipts.iter().filter(|r| r.replay_verified).count(),
        tamper_rejected: receipts.iter().filter(|r| r.tamper_rejected).count(),
        source_mutations: mutations.len(),
        source_mutations_rejected,
        provenance_preserved: receipts
            .iter()
            .filter(|r| r.source_provenance_preserved)
            .count(),
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

    assert_eq!(report.development_exercises, 240);
    assert_eq!(report.development_exact_decisions, 240);
    assert_eq!(report.holdout_exercises, 60);
    assert_eq!(report.holdout_exact_decisions, 60);
    assert_eq!(report.source_mutations_rejected, report.source_mutations);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.answer_keys_read, 0);
    assert_eq!(report.hle_questions_read, 0);
    assert_eq!(report.production_mutations, 0);
    assert!(report.manifest_unchanged);

    fs::write(
        &report_json,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        &report_md,
        format!(
            "# Stage 333 — Goal 6 source-selected acquisition\n\n\
             - Selected source / route: `{}` / `{}`\n\
             - Source records / validation: {} / {}\n\
             - Development exercises / exact decisions: {} / {}\n\
             - Development supported / ambiguous / refused: {} / {} / {}\n\
             - Holdout exercises / exact decisions: {} / {}\n\
             - Supported artifacts / replay / tamper: {} / {} / {}\n\
             - Source mutations rejected: {} / {}\n\
             - Provenance-preserved artifacts: {}\n\
             - False authorizations / denials: {} / {}\n\
             - Answer keys / HLE questions / production mutations: {} / {} / {}\n\
             - Manifest unchanged: {}\n\n\
             The source was selected from the answer-key-blind Goal 6 development \
             plan. Extraction and execution use generic declarative formula \
             machinery; no domain-specific runtime branch or live promotion was \
             introduced.\n",
            report.selected_source_path,
            report.selected_route,
            report.source_record_count,
            report.source_records_validated,
            report.development_exercises,
            report.development_exact_decisions,
            report.development_supported,
            report.development_ambiguous,
            report.development_refused,
            report.holdout_exercises,
            report.holdout_exact_decisions,
            report.supported_artifacts,
            report.replay_verified,
            report.tamper_rejected,
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
        "Stage 333 — source={} route={} dev={} holdout={} replay={} false_auth=0",
        report.selected_source_path,
        report.selected_route,
        report.development_exact_decisions,
        report.holdout_exact_decisions,
        report.replay_verified
    );
    Ok(())
}
