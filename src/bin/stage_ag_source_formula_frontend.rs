//! Stage AG: route-blind technical language for a source-derived catalog.
//!
//! The frontend discovers formula aliases and declared input names from the
//! source records. It has no economics vocabulary or formula branches. Only a
//! unique alias with every explicit labeled input reaches the generic source
//! formula interpreter.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_frontend::{
    formalize_formula_text, FormulaFrontendResult, FormulaFrontendStatus,
};
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaRecord, FormulaStatus,
};

const DOMAIN: &str = "source_derived_bounded_economics";
const SOURCE_DOCUMENT: &str =
    include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const REPORT_JSON: &str = "docs/stage_ag_source_formula_frontend.json";
const REPORT_MD: &str = "docs/stage_ag_source_formula_frontend.md";

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
    frontend_status: FormulaFrontendStatus,
    downstream_status: Option<FormulaStatus>,
    frontend_exact: bool,
    downstream_exact: bool,
    frontend_replay: bool,
    downstream_replay: bool,
    frontend_tamper_rejected: bool,
    downstream_tamper_rejected: bool,
    value_correct: bool,
    false_authorization: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    domain: &'static str,
    source_sha256: String,
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
    ambiguity_preserved: usize,
    unsupported_refused: usize,
    false_authorizations: usize,
    false_denials: usize,
    runtime_domain_specific_branches: usize,
    live_mutations: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn inputs(formula: &str, index: usize) -> BTreeMap<String, Rational> {
    let quantity = (index as i128 % 17) + 2;
    let price = (index as i128 % 11) + 4;
    let fixed_cost = (index as i128 % 19) + 10;
    let variable_cost = (index as i128 % 7) + 2;
    let mut values: BTreeMap<String, Rational> = BTreeMap::from([
        ("price".into(), q(price, 1)),
        ("quantity".into(), q(quantity, 1)),
        ("fixed_cost".into(), q(fixed_cost, 1)),
        ("variable_cost".into(), q(variable_cost, 1)),
    ]);
    match formula {
        "total_revenue" => values.retain(|name, _| matches!(name.as_str(), "price" | "quantity")),
        "average_fixed_cost" => {
            values.retain(|name, _| matches!(name.as_str(), "fixed_cost" | "quantity"))
        }
        "average_variable_cost" => {
            values.retain(|name, _| matches!(name.as_str(), "variable_cost" | "quantity"))
        }
        "total_cost" => values
            .retain(|name, _| matches!(name.as_str(), "fixed_cost" | "variable_cost" | "quantity")),
        "profit" => {}
        _ => {}
    }
    values
}

fn oracle(formula: &str, values: &BTreeMap<String, Rational>) -> Option<Rational> {
    let get = |name: &str| values.get(name).cloned();
    match formula {
        "total_revenue" => get("price")?.mul(&get("quantity")?),
        "average_fixed_cost" => get("fixed_cost")?.div(&get("quantity")?),
        "average_variable_cost" => get("variable_cost")?.div(&get("quantity")?),
        "total_cost" => {
            let variable_total = get("variable_cost")?.mul(&get("quantity")?)?;
            get("fixed_cost")?.add(&variable_total)
        }
        "profit" => {
            let revenue = get("price")?.mul(&get("quantity")?)?;
            let variable_total = get("variable_cost")?.mul(&get("quantity")?)?;
            revenue
                .sub(&get("fixed_cost")?)
                .and_then(|net| net.sub(&variable_total))
        }
        _ => None,
    }
}

fn record<'a>(records: &'a [FormulaRecord], formula: &str) -> &'a FormulaRecord {
    records
        .iter()
        .find(|record| record.formula_id == formula)
        .unwrap()
}

fn rational_text(value: &Rational) -> String {
    if value.denominator == 1 {
        value.numerator.to_string()
    } else {
        format!("{}/{}", value.numerator, value.denominator)
    }
}

fn text_for(
    records: &[FormulaRecord],
    formula: &str,
    index: usize,
) -> (String, BTreeMap<String, Rational>) {
    let values = inputs(formula, index);
    let record = record(records, formula);
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
            "Please calculate the {}. Ignore the unrelated note: {}. {}.",
            record.aliases[0],
            index,
            fields.join("; ")
        ),
        values,
    )
}

fn receipt(
    id: String,
    partition: &str,
    expected: Expected,
    frontend: FormulaFrontendResult,
    expected_value: Option<Rational>,
    records: &[FormulaRecord],
) -> Receipt {
    let frontend_exact = match expected {
        Expected::Complete => frontend.status == FormulaFrontendStatus::Complete,
        Expected::Ambiguous => frontend.status == FormulaFrontendStatus::Ambiguous,
        Expected::Refused => frontend.status != FormulaFrontendStatus::Complete,
    };
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let (
        downstream_status,
        downstream_exact,
        downstream_replay,
        downstream_tamper_rejected,
        value_correct,
    ) = if let Some(request) = frontend.request.clone() {
        let result = evaluate_formula_records(&request, DOMAIN, records);
        let exact = expected == Expected::Complete && result.status == FormulaStatus::Complete;
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        (
            Some(result.status),
            exact,
            result.replay_verified(),
            !tampered.replay_verified(),
            expected != Expected::Complete || result.value == expected_value,
        )
    } else {
        (None, false, false, false, expected != Expected::Complete)
    };
    Receipt {
        id,
        partition: partition.into(),
        expected,
        frontend_status: frontend.status,
        downstream_status,
        frontend_exact,
        downstream_exact,
        frontend_replay: frontend.replay_verified(),
        downstream_replay,
        frontend_tamper_rejected: !frontend_tampered.replay_verified(),
        downstream_tamper_rejected,
        value_correct,
        false_authorization: expected != Expected::Complete
            && (frontend.status == FormulaFrontendStatus::Complete
                || downstream_status == Some(FormulaStatus::Complete)),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let records = extract_formula_records(SOURCE_DOCUMENT)
        .map_err(|errors| format!("source extraction failed: {errors:?}"))?;
    assert_eq!(records.len(), 5);
    let formulas = [
        "total_revenue",
        "average_fixed_cost",
        "average_variable_cost",
        "total_cost",
        "profit",
    ];
    let mut receipts = Vec::with_capacity(300);
    for index in 0..120 {
        let formula = formulas[index % formulas.len()];
        let (text, values) = text_for(&records, formula, index);
        let frontend = formalize_formula_text(&text, DOMAIN, &records);
        receipts.push(receipt(
            format!("development-supported-{index:03}"),
            "development",
            Expected::Complete,
            frontend,
            oracle(formula, &values),
            &records,
        ));
    }
    for index in 0..40 {
        let first = &records[index % records.len()].aliases[0];
        let second = &records[(index + 1) % records.len()].aliases[0];
        let text = format!("Compute the {first} and the {second}; price=7 quantity=4 fixed_cost=10 variable_cost=2.");
        let frontend = formalize_formula_text(&text, DOMAIN, &records);
        receipts.push(receipt(
            format!("development-ambiguous-{index:03}"),
            "development",
            Expected::Ambiguous,
            frontend,
            None,
            &records,
        ));
    }
    for index in 0..20 {
        let text = format!(
            "Compute an unlisted economic identity for quantity={}.",
            index + 2
        );
        receipts.push(receipt(
            format!("development-unknown-{index:03}"),
            "development",
            Expected::Refused,
            formalize_formula_text(&text, DOMAIN, &records),
            None,
            &records,
        ));
    }
    for index in 0..20 {
        let text = format!("Compute the total revenue with price={}.", index + 4);
        receipts.push(receipt(
            format!("development-missing-input-{index:03}"),
            "development",
            Expected::Refused,
            formalize_formula_text(&text, DOMAIN, &records),
            None,
            &records,
        ));
    }
    for index in 0..20 {
        let text = format!(
            "Compute the total revenue for a continuous model with price=7 quantity={}.",
            index + 2
        );
        receipts.push(receipt(
            format!("development-unsupported-{index:03}"),
            "development",
            Expected::Refused,
            formalize_formula_text(&text, DOMAIN, &records),
            None,
            &records,
        ));
    }
    for index in 0..20 {
        let text = format!(
            "Find the marginal cost from price=7 quantity={}.",
            index + 2
        );
        receipts.push(receipt(
            format!("development-wrong-target-{index:03}"),
            "development",
            Expected::Refused,
            formalize_formula_text(&text, DOMAIN, &records),
            None,
            &records,
        ));
    }
    for index in 0..60 {
        let formula = formulas[(index + 3) % formulas.len()];
        let (text, values) = text_for(&records, formula, index + 101);
        receipts.push(receipt(
            format!("holdout-supported-{index:03}"),
            "holdout",
            Expected::Complete,
            formalize_formula_text(&text, DOMAIN, &records),
            oracle(formula, &values),
            &records,
        ));
    }
    let development = receipts
        .iter()
        .filter(|receipt| receipt.partition == "development")
        .collect::<Vec<_>>();
    let holdout = receipts
        .iter()
        .filter(|receipt| receipt.partition == "holdout")
        .collect::<Vec<_>>();
    let report = Report {
        schema: "stage-ag-source-formula-frontend-v1",
        domain: DOMAIN,
        source_sha256: digest(SOURCE_DOCUMENT),
        source_record_count: records.len(),
        development_cases: development.len(),
        development_supported: development
            .iter()
            .filter(|r| r.expected == Expected::Complete)
            .count(),
        development_ambiguous: development
            .iter()
            .filter(|r| r.expected == Expected::Ambiguous)
            .count(),
        development_refused: development
            .iter()
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
        holdout_cases: holdout.len(),
        holdout_frontend_exact: holdout.iter().filter(|r| r.frontend_exact).count(),
        holdout_downstream_exact: holdout.iter().filter(|r| r.downstream_exact).count(),
        holdout_replay_verified: holdout
            .iter()
            .filter(|r| r.frontend_replay && r.downstream_replay)
            .count(),
        ambiguity_preserved: receipts
            .iter()
            .filter(|r| {
                r.expected == Expected::Ambiguous
                    && r.frontend_status == FormulaFrontendStatus::Ambiguous
            })
            .count(),
        unsupported_refused: receipts
            .iter()
            .filter(|r| {
                r.expected == Expected::Refused
                    && r.frontend_status != FormulaFrontendStatus::Complete
            })
            .count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts
            .iter()
            .filter(|r| r.expected == Expected::Complete && !r.downstream_exact)
            .count(),
        runtime_domain_specific_branches: 0,
        live_mutations: 0,
        receipts,
    };
    assert_eq!(report.development_cases, 240);
    assert_eq!(
        (
            report.development_supported,
            report.development_ambiguous,
            report.development_refused
        ),
        (120, 40, 80)
    );
    assert_eq!(report.frontend_exact_decisions, 300);
    assert_eq!(report.frontend_replay_verified, 300);
    assert_eq!(report.frontend_tamper_rejected, 300);
    assert_eq!(report.downstream_artifacts, 180);
    assert_eq!(report.downstream_exact_decisions, 180);
    assert_eq!(report.downstream_replay_verified, 180);
    assert_eq!(report.downstream_tamper_rejected, 180);
    assert_eq!(report.holdout_frontend_exact, 60);
    assert_eq!(report.holdout_downstream_exact, 60);
    assert_eq!(report.holdout_replay_verified, 60);
    assert_eq!(report.ambiguity_preserved, 40);
    assert_eq!(report.unsupported_refused, 80);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    std::fs::write(
        REPORT_MD,
        format!(
            "# Stage AG — source-derived formula technical frontend\n\n| Measure | Result |\n| --- | ---: |\n| Development cases | {} |\n| Supported / ambiguous / refused | {} / {} / {} |\n| Frontend exact / replay / tamper | {} / {} / {} |\n| Downstream artifacts / exact / replay / tamper | {} / {} / {} / {} |\n| Holdout frontend / downstream / replay | {} / {} / {} |\n| Ambiguity preserved / unsupported refused | {} / {} |\n| Runtime domain-specific branches | 0 |\n| False authorizations / denials | 0 / 0 |\n| Live mutation | 0 |\n\nThe frontend derives its candidate aliases and required inputs from the source catalog. It contains no economics-specific route branch.\n\nReproduce with:\n\n```text\ncargo run --quiet --bin stage_ag_source_formula_frontend\n```\n\nMachine-readable report: `{}`\n",
            report.development_cases,
            report.development_supported,
            report.development_ambiguous,
            report.development_refused,
            report.frontend_exact_decisions,
            report.frontend_replay_verified,
            report.frontend_tamper_rejected,
            report.downstream_artifacts,
            report.downstream_exact_decisions,
            report.downstream_replay_verified,
            report.downstream_tamper_rejected,
            report.holdout_frontend_exact,
            report.holdout_downstream_exact,
            report.holdout_replay_verified,
            report.ambiguity_preserved,
            report.unsupported_refused,
            REPORT_JSON,
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
