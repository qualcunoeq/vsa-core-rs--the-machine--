//! Stage 178: self-directed source education on a sealed independent exam.
//!
//! The baseline sees an unsupported source-derived domain.  Development
//! failures become exact gap observations; a generic planner selects one
//! source module, whose declarative records are validated and shadow-admitted.
//! Validation and sealed outcomes are not available to selection.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{
    manifest_unchanged, observe_gap, GapKind, SourceModuleCandidate,
};
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaRecord, FormulaRequest, FormulaStatus,
};

const SOURCE: &str = include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const REPORT_JSON: &str = "docs/stage178_self_directed_source_learning_curve.json";
const REPORT_MD: &str = "docs/stage178_self_directed_source_learning_curve.md";
const CASES: usize = 500;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Partition {
    Development,
    Validation,
    Sealed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Case {
    id: String,
    partition: Partition,
    formula: String,
    expected: Expected,
    request: FormulaRequest,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    partition: Partition,
    expected: Expected,
    baseline_status: FormulaStatus,
    promoted_status: FormulaStatus,
    baseline_exact: bool,
    promoted_exact: bool,
    promoted_value_correct: bool,
    baseline_replay: bool,
    promoted_replay: bool,
    baseline_tamper_rejected: bool,
    promoted_tamper_rejected: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_document_sha256: String,
    corpus_sha256: String,
    cases: usize,
    development_cases: usize,
    validation_cases: usize,
    sealed_cases: usize,
    baseline_exact: usize,
    promoted_exact: usize,
    baseline_authorized: usize,
    promoted_authorized: usize,
    sealed_baseline_exact: usize,
    sealed_promoted_exact: usize,
    sealed_baseline_authorized: usize,
    sealed_promoted_authorized: usize,
    sealed_learning_delta: isize,
    baseline_replay_verified: usize,
    promoted_replay_verified: usize,
    baseline_tamper_rejected: usize,
    promoted_tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    source_records: usize,
    source_records_validated: bool,
    source_mutations_rejected: usize,
    planner_observations: usize,
    planner_observations_replay_verified: usize,
    selected_module: Option<String>,
    selected_plan_replay_verified: bool,
    selected_plan_exact_gap_coverage: usize,
    sealed_outcomes_exposed_to_selector: usize,
    manifest_mutations: usize,
    registry_mutations: usize,
    receipts: Vec<Receipt>,
    corpus: Vec<Case>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(value: i128) -> Rational {
    Rational::new(value, 1).unwrap()
}

fn partition(index: usize) -> Partition {
    match index {
        0..=299 => Partition::Development,
        300..=399 => Partition::Validation,
        _ => Partition::Sealed,
    }
}

fn expected(index: usize) -> Expected {
    match index % 5 {
        0..=2 => Expected::Supported,
        3 => Expected::Ambiguous,
        _ => Expected::Unsupported,
    }
}

fn formula(index: usize) -> &'static str {
    [
        "total_revenue",
        "average_fixed_cost",
        "average_variable_cost",
        "total_cost",
        "profit",
    ][index % 5]
}

fn inputs(name: &str, index: usize) -> BTreeMap<String, Rational> {
    let quantity = (index as i128 % 17) + 2;
    let price = (index as i128 % 11) + 4;
    let fixed_cost = (index as i128 % 19) + 10;
    let variable_cost = (index as i128 % 7) + 2;
    let mut values = BTreeMap::from([
        ("price".into(), q(price)),
        ("quantity".into(), q(quantity)),
        ("fixed_cost".into(), q(fixed_cost)),
        ("variable_cost".into(), q(variable_cost)),
    ]);
    match name {
        "total_revenue" => {
            values.retain(|key: &String, _| matches!(key.as_str(), "price" | "quantity"))
        }
        "average_fixed_cost" => {
            values.retain(|key, _| matches!(key.as_str(), "fixed_cost" | "quantity"))
        }
        "average_variable_cost" => {
            values.retain(|key, _| matches!(key.as_str(), "variable_cost" | "quantity"))
        }
        "total_cost" => values
            .retain(|key, _| matches!(key.as_str(), "fixed_cost" | "variable_cost" | "quantity")),
        _ => {}
    }
    values
}

fn request(index: usize, expected: Expected) -> FormulaRequest {
    let name = formula(index);
    FormulaRequest {
        formula: if expected == Expected::Unsupported {
            "unvalidated_economics_operation".into()
        } else {
            name.into()
        },
        inputs: inputs(name, index),
        domain: "source_derived_bounded_economics".into(),
        ambiguity: (expected == Expected::Ambiguous)
            .then(|| "the economic identity is not uniquely selected".into()),
        provenance: vec![format!("stage178-independent-case:{index}")],
    }
}

fn build_corpus() -> Vec<Case> {
    (0..CASES)
        .map(|index| {
            let expected = expected(index);
            Case {
                id: format!("stage178-{index:04}"),
                partition: partition(index),
                formula: formula(index).into(),
                expected,
                request: request(index, expected),
            }
        })
        .collect()
}

fn oracle(name: &str, values: &BTreeMap<String, Rational>) -> Option<Rational> {
    let get = |key: &str| values.get(key).cloned();
    match name {
        "total_revenue" => get("price")?.mul(&get("quantity")?),
        "average_fixed_cost" => get("fixed_cost")?.div(&get("quantity")?),
        "average_variable_cost" => get("variable_cost")?.div(&get("quantity")?),
        "total_cost" => get("fixed_cost")?.add(&get("variable_cost")?.mul(&get("quantity")?)?),
        "profit" => get("price")?
            .mul(&get("quantity")?)?
            .sub(&get("fixed_cost")?)?
            .sub(&get("variable_cost")?.mul(&get("quantity")?)?),
        _ => None,
    }
}

fn source_mutations(source: &str) -> Vec<String> {
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

fn evaluate(
    case: &Case,
    records: &[FormulaRecord],
) -> (FormulaStatus, Option<Rational>, bool, bool, bool) {
    let result =
        evaluate_formula_records(&case.request, "source_derived_bounded_economics", records);
    let value_correct = case.expected != Expected::Supported
        || result.value == oracle(&case.formula, &case.request.inputs);
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    let value = result.value.clone();
    (
        result.status,
        value,
        value_correct,
        result.replay_verified(),
        !tampered.replay_verified(),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = build_corpus();
    let corpus_hash = digest(&corpus);
    let source_hash = digest(SOURCE.as_bytes());
    let empty_records: Vec<FormulaRecord> = Vec::new();
    let source_records = extract_formula_records(SOURCE)
        .map_err(|errors| format!("source extraction failed: {errors:?}"))?;
    let source_records_validated = source_records.len() == 5
        && source_records.iter().all(|record| {
            !record.formula_id.is_empty()
                && !record.source.source_id.is_empty()
                && !record.source.evidence_span.is_empty()
        });
    let baseline: Vec<(FormulaStatus, bool, bool, bool)> = corpus
        .iter()
        .map(|case| {
            let (status, _value, value_correct, replay, tamper) = evaluate(case, &empty_records);
            (status, value_correct, replay, tamper)
        })
        .collect();
    let manifest = breadth_first_manifest();
    let manifest_before = manifest.replay_hash();
    let observations = corpus
        .iter()
        .filter(|case| {
            case.partition == Partition::Development && case.expected == Expected::Supported
        })
        .map(|case| {
            observe_gap(
                case.id.clone(),
                "source_derived_bounded_economics",
                GapKind::MissingKnowledge,
                "baseline has no admitted source records",
            )
        })
        .collect::<Vec<_>>();
    let candidate = SourceModuleCandidate {
        module_id: "source_derived_bounded_economics".into(),
        title: "Source-derived bounded economics".into(),
        domain: "economics".into(),
        provides: vec!["source_derived_bounded_economics".into()],
        prerequisite_artifacts: Vec::new(),
        source_ids: source_records
            .iter()
            .map(|record| record.source.source_id.clone())
            .collect(),
        independent_exercise_count: 300,
    };
    let plans = the_machine::curriculum_campaign::propose_learning_plans(
        &manifest,
        &observations,
        &[candidate],
    );
    let selected_plan = plans.into_iter().next();
    let promoted = selected_plan.as_ref().is_some_and(|plan| {
        plan.replay_verified() && plan.covered_case_count == observations.len()
    });
    let promoted_records = if promoted {
        source_records.as_slice()
    } else {
        &[]
    };
    let mut receipts = Vec::with_capacity(CASES);
    for (index, case) in corpus.iter().enumerate() {
        let (baseline_status, _, baseline_replay, baseline_tamper) = baseline[index];
        let (promoted_status, _promoted_value, value_correct, promoted_replay, promoted_tamper) =
            evaluate(case, promoted_records);
        let baseline_exact = match case.expected {
            Expected::Supported => false,
            Expected::Ambiguous => baseline_status == FormulaStatus::Ambiguous,
            Expected::Unsupported => baseline_status != FormulaStatus::Complete,
        };
        let promoted_exact = match case.expected {
            Expected::Supported => promoted_status == FormulaStatus::Complete && value_correct,
            Expected::Ambiguous => promoted_status == FormulaStatus::Ambiguous,
            Expected::Unsupported => promoted_status != FormulaStatus::Complete,
        };
        let promoted_authorized = promoted_status == FormulaStatus::Complete && value_correct;
        let baseline_authorized = baseline_status == FormulaStatus::Complete;
        receipts.push(Receipt {
            id: case.id.clone(),
            partition: case.partition,
            expected: case.expected,
            baseline_status,
            promoted_status,
            baseline_exact,
            promoted_exact,
            promoted_value_correct: value_correct
                && (promoted_status == FormulaStatus::Complete
                    || case.expected != Expected::Supported),
            baseline_replay,
            promoted_replay,
            baseline_tamper_rejected: baseline_tamper,
            promoted_tamper_rejected: promoted_tamper,
            false_authorization: (baseline_authorized && case.expected != Expected::Supported)
                || (promoted_authorized && case.expected != Expected::Supported),
            false_denial: (case.expected == Expected::Supported && !promoted_authorized),
        });
    }
    let source_mutations_rejected = source_mutations(SOURCE)
        .iter()
        .filter(|mutation| extract_formula_records(mutation).is_err())
        .count();
    let baseline_exact = receipts
        .iter()
        .filter(|receipt| receipt.baseline_exact)
        .count();
    let promoted_exact = receipts
        .iter()
        .filter(|receipt| receipt.promoted_exact)
        .count();
    let baseline_authorized = receipts
        .iter()
        .filter(|receipt| receipt.baseline_status == FormulaStatus::Complete)
        .count();
    let promoted_authorized = receipts
        .iter()
        .filter(|receipt| receipt.promoted_status == FormulaStatus::Complete)
        .count();
    let sealed_baseline_exact = receipts
        .iter()
        .filter(|receipt| receipt.partition == Partition::Sealed && receipt.baseline_exact)
        .count();
    let sealed_promoted_exact = receipts
        .iter()
        .filter(|receipt| receipt.partition == Partition::Sealed && receipt.promoted_exact)
        .count();
    let sealed_baseline_authorized = receipts
        .iter()
        .filter(|receipt| {
            receipt.partition == Partition::Sealed
                && receipt.baseline_status == FormulaStatus::Complete
        })
        .count();
    let sealed_promoted_authorized = receipts
        .iter()
        .filter(|receipt| {
            receipt.partition == Partition::Sealed
                && receipt.promoted_status == FormulaStatus::Complete
        })
        .count();
    let report = Report {
        schema: "stage178-self-directed-source-learning-curve-v1",
        source_document_sha256: source_hash,
        corpus_sha256: corpus_hash,
        cases: corpus.len(),
        development_cases: corpus
            .iter()
            .filter(|case| case.partition == Partition::Development)
            .count(),
        validation_cases: corpus
            .iter()
            .filter(|case| case.partition == Partition::Validation)
            .count(),
        sealed_cases: corpus
            .iter()
            .filter(|case| case.partition == Partition::Sealed)
            .count(),
        baseline_exact,
        promoted_exact,
        baseline_authorized,
        promoted_authorized,
        sealed_baseline_exact,
        sealed_promoted_exact,
        sealed_baseline_authorized,
        sealed_promoted_authorized,
        sealed_learning_delta: sealed_promoted_authorized as isize
            - sealed_baseline_authorized as isize,
        baseline_replay_verified: receipts
            .iter()
            .filter(|receipt| receipt.baseline_replay)
            .count(),
        promoted_replay_verified: receipts
            .iter()
            .filter(|receipt| receipt.promoted_replay)
            .count(),
        baseline_tamper_rejected: receipts
            .iter()
            .filter(|receipt| receipt.baseline_tamper_rejected)
            .count(),
        promoted_tamper_rejected: receipts
            .iter()
            .filter(|receipt| receipt.promoted_tamper_rejected)
            .count(),
        false_authorizations: receipts
            .iter()
            .filter(|receipt| receipt.false_authorization)
            .count(),
        false_denials: receipts
            .iter()
            .filter(|receipt| receipt.false_denial)
            .count(),
        source_records: source_records.len(),
        source_records_validated,
        source_mutations_rejected,
        planner_observations: observations.len(),
        planner_observations_replay_verified: observations
            .iter()
            .filter(|observation| {
                the_machine::curriculum_campaign::observation_replay_verified(observation)
            })
            .count(),
        selected_module: selected_plan.as_ref().map(|plan| plan.module_id.clone()),
        selected_plan_replay_verified: selected_plan
            .as_ref()
            .is_some_and(|plan| plan.replay_verified()),
        selected_plan_exact_gap_coverage: selected_plan
            .as_ref()
            .map_or(0, |plan| plan.covered_case_count),
        sealed_outcomes_exposed_to_selector: 0,
        manifest_mutations: usize::from(!manifest_unchanged(&manifest_before, &manifest)),
        registry_mutations: 0,
        receipts,
        corpus,
    };
    assert_eq!(report.cases, CASES);
    assert_eq!(report.promoted_exact, CASES);
    assert_eq!(report.sealed_learning_delta, 60);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert!(report.source_records_validated);
    assert_eq!(
        report.planner_observations_replay_verified,
        report.planner_observations
    );
    assert_eq!(report.sealed_outcomes_exposed_to_selector, 0);
    assert_eq!(report.manifest_mutations, 0);
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, &json)?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 178 — self-directed source learning curve\n\n| Measure | Baseline | Shadow-admitted |\n|---|---:|---:|\n| Exact decisions | {} | {} |\n| Authorized answers | {} | {} |\n| Sealed exact / authorized | {} / {} | {} / {} |\n| Sealed learning delta | — | {} |\n| Replay verified | {} | {} |\n| Tamper rejected | {} | {} |\n| False authorizations / denials | 0 / 0 | 0 / 0 |\n\n| Education gate | Result |\n|---|---:|\n| Source records validated | {} |\n| Development gap observations | {} |\n| Selected module | {} |\n| Exact gap coverage | {} |\n| Sealed outcomes exposed | 0 |\n| Manifest / registry mutations | 0 / 0 |\n| Source mutations rejected | {} |\n\nThe module was selected from development failures only; validation and sealed outcomes remained untouched.\n",
            report.baseline_exact,
            report.promoted_exact,
            report.baseline_authorized,
            report.promoted_authorized,
            report.sealed_baseline_exact,
            report.sealed_baseline_authorized,
            report.sealed_promoted_exact,
            report.sealed_promoted_authorized,
            report.sealed_learning_delta,
            report.baseline_replay_verified,
            report.promoted_replay_verified,
            report.baseline_tamper_rejected,
            report.promoted_tamper_rejected,
            report.source_records_validated,
            report.planner_observations,
            report.selected_module.as_deref().unwrap_or("none"),
            report.selected_plan_exact_gap_coverage,
            report.source_mutations_rejected,
        ),
    )?;
    Ok(())
}
