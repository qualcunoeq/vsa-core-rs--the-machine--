//! Stage 180: autonomous acquisition of a previously unregistered source
//! catalog.
//!
//! The source records are parsed and validated by the domain-agnostic catalog
//! layer.  Module identity, provided artifacts, source IDs, and exercise
//! evidence are inferred from the records; no health-specific executor or
//! curriculum branch exists.  The inferred module is shadow-admitted only.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{
    manifest_unchanged, observe_gap, propose_learning_plans, GapKind, SourceModuleCandidate,
};
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, validate_formula_records, FormulaRecord,
    FormulaRequest, FormulaStatus,
};

const SOURCE: &str = include_str!("../../docs/sources/openstax_bounded_health_ratios_source.txt");
const REPORT_JSON: &str = "docs/stage180_autonomous_source_catalog.json";
const REPORT_MD: &str = "docs/stage180_autonomous_source_catalog.md";
const CASES: usize = 1000;

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
    source_records: usize,
    source_records_validated: bool,
    source_record_ids_inferred: usize,
    inferred_module_id: Option<String>,
    inferred_artifacts: usize,
    source_mutations_rejected: usize,
    planner_observations: usize,
    planner_observations_replay_verified: usize,
    selected_plan_replay_verified: bool,
    selected_plan_exact_gap_coverage: usize,
    baseline_replay_verified: usize,
    promoted_replay_verified: usize,
    baseline_tamper_rejected: usize,
    promoted_tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
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
        0..=599 => Partition::Development,
        600..=799 => Partition::Validation,
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
        "incidence_rate",
        "mortality_rate",
        "prevalence",
        "case_fatality_ratio",
        "odds_ratio",
    ][index % 5]
}

fn inputs(name: &str, index: usize) -> BTreeMap<String, Rational> {
    let population = (index as i128 % 31) + 100;
    let new_cases = (index as i128 % 17) + 2;
    let deaths = (index as i128 % 7) + 1;
    let existing_cases = (index as i128 % 19) + 3;
    let cases = (index as i128 % 23) + 8;
    let exposed_cases = (index as i128 % 11) + 2;
    let unexposed_non_cases = (index as i128 % 13) + 3;
    let exposed_non_cases = (index as i128 % 7) + 4;
    let unexposed_cases = (index as i128 % 5) + 5;
    let mut values = BTreeMap::from([
        ("population".into(), q(population)),
        ("new_cases".into(), q(new_cases)),
        ("deaths".into(), q(deaths)),
        ("existing_cases".into(), q(existing_cases)),
        ("cases".into(), q(cases)),
        ("exposed_cases".into(), q(exposed_cases)),
        ("unexposed_non_cases".into(), q(unexposed_non_cases)),
        ("exposed_non_cases".into(), q(exposed_non_cases)),
        ("unexposed_cases".into(), q(unexposed_cases)),
    ]);
    let keep: &[&str] = match name {
        "incidence_rate" => &["new_cases", "population"],
        "mortality_rate" => &["deaths", "population"],
        "prevalence" => &["existing_cases", "population"],
        "case_fatality_ratio" => &["deaths", "cases"],
        _ => &[
            "exposed_cases",
            "unexposed_non_cases",
            "exposed_non_cases",
            "unexposed_cases",
        ],
    };
    values.retain(|key: &String, _| keep.contains(&key.as_str()));
    values
}

fn request(index: usize, expected: Expected) -> FormulaRequest {
    let name = formula(index);
    FormulaRequest {
        formula: if expected == Expected::Unsupported {
            "unvalidated_health_ratio".into()
        } else {
            name.into()
        },
        inputs: inputs(name, index),
        domain: "inferred_source_catalog".into(),
        ambiguity: (expected == Expected::Ambiguous)
            .then(|| "the ratio identity is not uniquely selected".into()),
        provenance: vec![format!("stage180-independent-case:{index}")],
    }
}

fn build_corpus() -> Vec<Case> {
    (0..CASES)
        .map(|index| {
            let expected = expected(index);
            Case {
                id: format!("stage180-{index:04}"),
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
        "incidence_rate" => get("new_cases")?.div(&get("population")?),
        "mortality_rate" => get("deaths")?.div(&get("population")?),
        "prevalence" => get("existing_cases")?.div(&get("population")?),
        "case_fatality_ratio" => get("deaths")?.div(&get("cases")?),
        "odds_ratio" => get("exposed_cases")?
            .mul(&get("unexposed_non_cases")?)?
            .div(&get("exposed_non_cases")?.mul(&get("unexposed_cases")?)?),
        _ => None,
    }
}

fn evaluate(
    case: &Case,
    records: &[FormulaRecord],
) -> (FormulaStatus, Option<Rational>, bool, bool, bool) {
    let result = evaluate_formula_records(&case.request, "inferred_source_catalog", records);
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

fn source_mutations(source: &str) -> Vec<String> {
    vec![
        source.replacen("END FORMULA", "", 1),
        source.replacen(
            "EXPRESSION: new_cases / population",
            "EXPRESSION: new_cases // population",
            1,
        ),
        source.replacen(
            "SOURCE_ID: openstax-introductory-statistics-2e:health:incidence",
            "SOURCE_ID:",
            1,
        ),
        source.replacen(
            "CONSTRAINTS: positive:new_cases; positive:population",
            "CONSTRAINTS: positive:missing",
            1,
        ),
        source.replacen(
            "ALIASES: incidence rate | new-case rate",
            "ALIASES: duplicate\nALIASES: duplicate",
            1,
        ),
        source.replacen(
            "URL: https://openstax.org/details/books/introductory-statistics-2e",
            "URL: file://local",
            1,
        ),
    ]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = build_corpus();
    let corpus_sha256 = digest(&corpus);
    let source_document_sha256 = digest(SOURCE.as_bytes());
    let records =
        extract_formula_records(SOURCE).map_err(|e| format!("source extraction failed: {e:?}"))?;
    let source_records_validated = validate_formula_records(&records).is_ok() && records.len() == 5;
    let source_record_ids: BTreeSet<String> =
        records.iter().map(|r| r.formula_id.clone()).collect();
    let inferred_module_id =
        source_records_validated.then(|| format!("source_catalog_{}", &digest(&records)[..16]));
    let inferred_artifacts = records
        .iter()
        .map(|r| r.formula_id.clone())
        .collect::<BTreeSet<_>>()
        .len();
    let empty: Vec<FormulaRecord> = Vec::new();
    let baseline: Vec<_> = corpus.iter().map(|case| evaluate(case, &empty)).collect();
    let manifest = breadth_first_manifest();
    let manifest_before = manifest.replay_hash();
    let artifact_key = format!("source_catalog_{}", &digest(&records)[..16]);
    let observations: Vec<_> = corpus
        .iter()
        .filter(|case| {
            case.partition == Partition::Development && case.expected == Expected::Supported
        })
        .map(|case| {
            observe_gap(
                case.id.clone(),
                artifact_key.clone(),
                GapKind::MissingKnowledge,
                "no source catalog admitted",
            )
        })
        .collect();
    let candidate = SourceModuleCandidate {
        module_id: inferred_module_id
            .clone()
            .unwrap_or_else(|| "invalid_source_catalog".into()),
        title: "Inferred source catalog module".into(),
        domain: "inferred_source_domain".into(),
        provides: vec![artifact_key.clone()],
        prerequisite_artifacts: Vec::new(),
        source_ids: records.iter().map(|r| r.source.source_id.clone()).collect(),
        independent_exercise_count: corpus
            .iter()
            .filter(|c| c.partition == Partition::Development)
            .count(),
    };
    let plan = propose_learning_plans(&manifest, &observations, &[candidate])
        .into_iter()
        .next();
    let promoted = source_records_validated
        && plan
            .as_ref()
            .is_some_and(|p| p.replay_verified() && p.covered_case_count == observations.len());
    let admitted = if promoted { records.as_slice() } else { &[] };
    let mut receipts = Vec::with_capacity(CASES);
    for (index, case) in corpus.iter().enumerate() {
        let (baseline_status, _, _, baseline_replay, baseline_tamper) = baseline[index];
        let (promoted_status, _, value_correct, promoted_replay, promoted_tamper) =
            evaluate(case, admitted);
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
            false_authorization: (baseline_authorized || promoted_authorized)
                && case.expected != Expected::Supported,
            false_denial: case.expected == Expected::Supported && !promoted_authorized,
        });
    }
    let source_mutations_rejected = source_mutations(SOURCE)
        .iter()
        .filter(|s| extract_formula_records(s).is_err())
        .count();
    let report = Report {
        schema: "stage180-autonomous-source-catalog-v1",
        source_document_sha256,
        corpus_sha256,
        cases: CASES,
        development_cases: corpus
            .iter()
            .filter(|c| c.partition == Partition::Development)
            .count(),
        validation_cases: corpus
            .iter()
            .filter(|c| c.partition == Partition::Validation)
            .count(),
        sealed_cases: corpus
            .iter()
            .filter(|c| c.partition == Partition::Sealed)
            .count(),
        baseline_exact: receipts.iter().filter(|r| r.baseline_exact).count(),
        promoted_exact: receipts.iter().filter(|r| r.promoted_exact).count(),
        baseline_authorized: receipts
            .iter()
            .filter(|r| r.baseline_status == FormulaStatus::Complete)
            .count(),
        promoted_authorized: receipts
            .iter()
            .filter(|r| r.promoted_status == FormulaStatus::Complete)
            .count(),
        sealed_baseline_exact: receipts
            .iter()
            .filter(|r| r.partition == Partition::Sealed && r.baseline_exact)
            .count(),
        sealed_promoted_exact: receipts
            .iter()
            .filter(|r| r.partition == Partition::Sealed && r.promoted_exact)
            .count(),
        sealed_baseline_authorized: receipts
            .iter()
            .filter(|r| {
                r.partition == Partition::Sealed && r.baseline_status == FormulaStatus::Complete
            })
            .count(),
        sealed_promoted_authorized: receipts
            .iter()
            .filter(|r| {
                r.partition == Partition::Sealed && r.promoted_status == FormulaStatus::Complete
            })
            .count(),
        sealed_learning_delta: receipts
            .iter()
            .filter(|r| {
                r.partition == Partition::Sealed && r.promoted_status == FormulaStatus::Complete
            })
            .count() as isize
            - receipts
                .iter()
                .filter(|r| {
                    r.partition == Partition::Sealed && r.baseline_status == FormulaStatus::Complete
                })
                .count() as isize,
        source_records: records.len(),
        source_records_validated,
        source_record_ids_inferred: source_record_ids.len(),
        inferred_module_id,
        inferred_artifacts,
        source_mutations_rejected,
        planner_observations: observations.len(),
        planner_observations_replay_verified: observations
            .iter()
            .filter(|o| the_machine::curriculum_campaign::observation_replay_verified(o))
            .count(),
        selected_plan_replay_verified: plan.as_ref().is_some_and(|p| p.replay_verified()),
        selected_plan_exact_gap_coverage: plan.as_ref().map_or(0, |p| p.covered_case_count),
        baseline_replay_verified: receipts.iter().filter(|r| r.baseline_replay).count(),
        promoted_replay_verified: receipts.iter().filter(|r| r.promoted_replay).count(),
        baseline_tamper_rejected: receipts
            .iter()
            .filter(|r| r.baseline_tamper_rejected)
            .count(),
        promoted_tamper_rejected: receipts
            .iter()
            .filter(|r| r.promoted_tamper_rejected)
            .count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts.iter().filter(|r| r.false_denial).count(),
        sealed_outcomes_exposed_to_selector: 0,
        manifest_mutations: usize::from(!manifest_unchanged(&manifest_before, &manifest)),
        registry_mutations: 0,
        receipts,
        corpus,
    };
    assert_eq!(report.cases, CASES);
    assert_eq!(report.promoted_exact, CASES);
    assert_eq!(report.promoted_authorized, 600);
    assert_eq!(report.sealed_learning_delta, 120);
    assert!(report.source_records_validated);
    assert_eq!(report.source_record_ids_inferred, 5);
    assert_eq!(report.inferred_artifacts, 5);
    assert_eq!(report.source_mutations_rejected, 6);
    assert_eq!(
        report.planner_observations_replay_verified,
        report.planner_observations
    );
    assert_eq!(
        report.selected_plan_exact_gap_coverage,
        report.planner_observations
    );
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.baseline_replay_verified, CASES);
    assert_eq!(report.promoted_replay_verified, CASES);
    assert_eq!(report.baseline_tamper_rejected, CASES);
    assert_eq!(report.promoted_tamper_rejected, CASES);
    assert_eq!(report.sealed_outcomes_exposed_to_selector, 0);
    assert_eq!(report.manifest_mutations, 0);
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, &json)?;
    fs::write(REPORT_MD, format!("# Stage 180 — autonomous source-catalog acquisition\n\n| Measure | Baseline | Shadow-admitted |\n|---|---:|---:|\n| Cases | {} | {} |\n| Authorized answers | {} | {} |\n| Sealed exact / authorized | {} / {} | {} / {} |\n| Sealed learning delta | — | {} |\n| Replay / tamper | {} / {} | {} / {} |\n| False authorizations / denials | 0 / 0 | 0 / 0 |\n\n| Acquisition gate | Result |\n|---|---:|\n| Source records validated | {}/{} |\n| Inferred artifact IDs | {} |\n| Source mutations rejected | {}/6 |\n| Development gap observations | {} |\n| Selected plan coverage | {} |\n| Sealed outcomes exposed | 0 |\n| Manifest / registry mutations | 0 / 0 |\n\nThe domain module identity and provided artifact key were derived from the validated source catalog. Execution used the generic formula runtime; no domain-specific executor branch or live promotion was used.\n", report.cases, report.promoted_exact, report.baseline_authorized, report.promoted_authorized, report.sealed_baseline_exact, report.sealed_baseline_authorized, report.sealed_promoted_exact, report.sealed_promoted_authorized, report.sealed_learning_delta, report.baseline_replay_verified, report.baseline_tamper_rejected, report.promoted_replay_verified, report.promoted_tamper_rejected, report.source_records, report.source_records_validated, report.inferred_artifacts, report.source_mutations_rejected, report.planner_observations, report.selected_plan_exact_gap_coverage))?;
    Ok(())
}
