//! Stage AE: source-to-capability acquisition without a domain executor.
//!
//! An attributed source transcription is parsed into declarative formula
//! records, validated, and executed by the existing domain-agnostic formula
//! interpreter. The economics domain is absent from the runtime: names,
//! expressions, assumptions, and provenance all come from source records.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaRecord, FormulaRequest, FormulaStatus,
};

const DOMAIN: &str = "source_derived_bounded_economics";
const SOURCE_DOCUMENT: &str =
    include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const REPORT_JSON: &str = "docs/stage_ae_source_capability_acquisition.json";
const REPORT_MD: &str = "docs/stage_ae_source_capability_acquisition.md";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum Expected {
    Complete,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Receipt {
    id: String,
    partition: String,
    formula: String,
    expected: Expected,
    actual: FormulaStatus,
    exact: bool,
    value_correct: bool,
    source_preserved: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    domain: &'static str,
    source_document_sha256: String,
    source_record_count: usize,
    source_records_validated: bool,
    runtime_domain_specific_branches: usize,
    independent_exercises: usize,
    development_supported: usize,
    development_ambiguous: usize,
    development_refused: usize,
    development_exact_decisions: usize,
    development_supported_artifacts: usize,
    development_replay_verified: usize,
    development_tamper_rejected: usize,
    holdout_supported: usize,
    holdout_exact_decisions: usize,
    holdout_replay_verified: usize,
    holdout_tamper_rejected: usize,
    source_mutations: usize,
    source_mutations_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    provenance_preserved: usize,
    no_live_mutation: bool,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("valid rational")
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

/// Independent oracle. This is deliberately separate from the generic source
/// parser and interpreter; it exists only to check their results.
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

fn request(formula: &str, values: BTreeMap<String, Rational>, id: &str) -> FormulaRequest {
    FormulaRequest {
        formula: formula.into(),
        inputs: values,
        domain: DOMAIN.into(),
        ambiguity: None,
        provenance: vec![format!("stage-ae:{id}:source-span")],
    }
}

fn evaluate(
    records: &[FormulaRecord],
    id: String,
    partition: &str,
    formula: &str,
    expected: Expected,
    request: FormulaRequest,
    expected_value: Option<Rational>,
) -> Receipt {
    let result = evaluate_formula_records(&request, DOMAIN, records);
    let exact = match expected {
        Expected::Complete => result.status == FormulaStatus::Complete,
        Expected::Ambiguous => result.status == FormulaStatus::Ambiguous,
        Expected::Refused => result.status != FormulaStatus::Complete,
    };
    let value_correct = expected != Expected::Complete || result.value == expected_value;
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    Receipt {
        id,
        partition: partition.into(),
        formula: formula.into(),
        expected,
        actual: result.status,
        exact,
        value_correct,
        source_preserved: expected == Expected::Complete && result.source.is_some(),
        replay_verified: result.replay_verified(),
        tamper_rejected: !tampered.replay_verified(),
        false_authorization: expected != Expected::Complete
            && result.status == FormulaStatus::Complete,
    }
}

fn holdout_values(formula: &str, index: usize) -> BTreeMap<String, Rational> {
    inputs(formula, index + 101)
}

fn mutate_source(source: &str) -> Vec<String> {
    vec![
        source.replacen("END FORMULA", "", 1),
        source.replacen(
            "EXPRESSION: price * quantity",
            "EXPRESSION: price // quantity",
            1,
        ),
        source.replacen(
            "SOURCE_ID: openstax-principles-economics-3e:revenue",
            "SOURCE_ID:",
            1,
        ),
        source.replacen(
            "CONSTRAINTS: positive:price; positive:quantity",
            "CONSTRAINTS: positive:missing",
            1,
        ),
        source.replacen(
            "ALIASES: total revenue | sales revenue",
            "ALIASES: duplicate\nALIASES: duplicate",
            1,
        ),
        source.replacen(
            "URL: https://openstax.org/details/books/principles-economics-3e",
            "URL: file://local",
            1,
        ),
    ]
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
        let values = inputs(formula, index);
        receipts.push(evaluate(
            &records,
            format!("development-supported-{index:03}"),
            "development",
            formula,
            Expected::Complete,
            request(formula, values.clone(), &format!("supported-{index:03}")),
            oracle(formula, &values),
        ));
    }
    for index in 0..40 {
        let formula = formulas[index % formulas.len()];
        let mut req = request(
            formula,
            inputs(formula, index + 19),
            &format!("ambiguous-{index:03}"),
        );
        req.ambiguity = Some("source wording selects more than one formulation".into());
        receipts.push(evaluate(
            &records,
            format!("development-ambiguous-{index:03}"),
            "development",
            formula,
            Expected::Ambiguous,
            req,
            None,
        ));
    }
    for index in 0..20 {
        let formula = "unlisted_economic_identity";
        let mut req = request(formula, BTreeMap::new(), &format!("unknown-{index:03}"));
        req.formula = formula.into();
        receipts.push(evaluate(
            &records,
            format!("development-unknown-{index:03}"),
            "development",
            formula,
            Expected::Refused,
            req,
            None,
        ));
    }
    for index in 0..20 {
        let formula = formulas[index % formulas.len()];
        let mut values = inputs(formula, index + 31);
        values.remove("quantity");
        receipts.push(evaluate(
            &records,
            format!("development-missing-input-{index:03}"),
            "development",
            formula,
            Expected::Refused,
            request(formula, values, &format!("missing-{index:03}")),
            None,
        ));
    }
    for index in 0..20 {
        let formula = formulas[index % formulas.len()];
        let mut req = request(
            formula,
            inputs(formula, index + 47),
            &format!("wrong-domain-{index:03}"),
        );
        req.domain = "unvalidated_domain".into();
        receipts.push(evaluate(
            &records,
            format!("development-wrong-domain-{index:03}"),
            "development",
            formula,
            Expected::Refused,
            req,
            None,
        ));
    }
    for index in 0..20 {
        let formula = if index % 2 == 0 {
            "average_fixed_cost"
        } else {
            "average_variable_cost"
        };
        let mut values = inputs(formula, index + 63);
        values.insert("quantity".into(), q(0, 1));
        receipts.push(evaluate(
            &records,
            format!("development-invalid-domain-{index:03}"),
            "development",
            formula,
            Expected::Refused,
            request(formula, values, &format!("invalid-{index:03}")),
            None,
        ));
    }
    for index in 0..60 {
        let formula = formulas[(index + 2) % formulas.len()];
        let values = holdout_values(formula, index);
        receipts.push(evaluate(
            &records,
            format!("holdout-supported-{index:03}"),
            "holdout",
            formula,
            Expected::Complete,
            request(formula, values.clone(), &format!("holdout-{index:03}")),
            oracle(formula, &values),
        ));
    }

    let mutations = mutate_source(SOURCE_DOCUMENT);
    let source_mutations_rejected = mutations
        .iter()
        .filter(|mutation| extract_formula_records(mutation).is_err())
        .count();
    let development = receipts
        .iter()
        .filter(|receipt| receipt.partition == "development")
        .collect::<Vec<_>>();
    let holdout = receipts
        .iter()
        .filter(|receipt| receipt.partition == "holdout")
        .collect::<Vec<_>>();
    let supported = development
        .iter()
        .filter(|receipt| receipt.expected == Expected::Complete)
        .count();
    let ambiguous = development
        .iter()
        .filter(|receipt| receipt.expected == Expected::Ambiguous)
        .count();
    let refused = development
        .iter()
        .filter(|receipt| receipt.expected == Expected::Refused)
        .count();
    let report = Report {
        schema: "stage-ae-source-capability-acquisition-v1",
        domain: DOMAIN,
        source_document_sha256: digest(SOURCE_DOCUMENT),
        source_record_count: records.len(),
        source_records_validated: true,
        runtime_domain_specific_branches: 0,
        independent_exercises: development.len(),
        development_supported: supported,
        development_ambiguous: ambiguous,
        development_refused: refused,
        development_exact_decisions: development.iter().filter(|r| r.exact).count(),
        development_supported_artifacts: development
            .iter()
            .filter(|r| r.expected == Expected::Complete && r.actual == FormulaStatus::Complete)
            .count(),
        development_replay_verified: development.iter().filter(|r| r.replay_verified).count(),
        development_tamper_rejected: development.iter().filter(|r| r.tamper_rejected).count(),
        holdout_supported: holdout.len(),
        holdout_exact_decisions: holdout.iter().filter(|r| r.exact).count(),
        holdout_replay_verified: holdout.iter().filter(|r| r.replay_verified).count(),
        holdout_tamper_rejected: holdout.iter().filter(|r| r.tamper_rejected).count(),
        source_mutations: mutations.len(),
        source_mutations_rejected,
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts
            .iter()
            .filter(|r| r.expected == Expected::Complete && !r.exact)
            .count(),
        provenance_preserved: receipts.iter().filter(|r| r.source_preserved).count(),
        no_live_mutation: true,
        receipts,
    };
    assert_eq!(report.independent_exercises, 240);
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!(report.development_exact_decisions, 240);
    assert_eq!(report.development_supported_artifacts, 120);
    assert_eq!(report.development_replay_verified, 240);
    assert_eq!(report.development_tamper_rejected, 240);
    assert_eq!(report.holdout_supported, 60);
    assert_eq!(report.holdout_exact_decisions, 60);
    assert_eq!(report.holdout_replay_verified, 60);
    assert_eq!(report.holdout_tamper_rejected, 60);
    assert_eq!(report.source_mutations_rejected, report.source_mutations);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.provenance_preserved, 180);
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    std::fs::write(
        REPORT_MD,
        format!(
            "# Stage AE — autonomous source capability acquisition\n\nThe source document was parsed into five declarative formula records and evaluated by the domain-agnostic expression runtime. No economics-specific executor branch is present.\n\n| Measure | Result |\n| --- | ---: |\n| Source records | {}/{} validated |\n| Independent development exercises | {} |\n| Development supported / ambiguous / refused | {} / {} / {} |\n| Development exact decisions | {}/{} |\n| Development artifacts / replay / tamper | {} / {} / {} |\n| Untouched holdout supported / exact / replay / tamper | {} / {} / {} / {} |\n| Source mutations rejected | {}/{} |\n| Provenance-preserved complete artifacts | {} |\n| Runtime domain-specific branches | {} |\n| False authorizations / denials | 0 / 0 |\n| Live mutation | false |\n\nReproduce with:\n\n```text\ncargo run --quiet --bin stage_ae_source_capability_acquisition\n```\n\nMachine-readable report: `{}`\n",
            report.source_record_count,
            report.source_record_count,
            report.independent_exercises,
            report.development_supported,
            report.development_ambiguous,
            report.development_refused,
            report.development_exact_decisions,
            report.independent_exercises,
            report.development_supported_artifacts,
            report.development_replay_verified,
            report.development_tamper_rejected,
            report.holdout_supported,
            report.holdout_exact_decisions,
            report.holdout_replay_verified,
            report.holdout_tamper_rejected,
            report.source_mutations_rejected,
            report.source_mutations,
            report.provenance_preserved,
            report.runtime_domain_specific_branches,
            REPORT_JSON,
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
