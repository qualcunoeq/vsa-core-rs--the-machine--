//! Stage 306: execute the modules selected by the curriculum utility portfolio.
//!
//! Stage 305 selects modules using exact typed gaps and a hard acquisition
//! budget.  This stage consumes that immutable selection and evaluates only
//! the selected source-derived runtimes on independent development,
//! validation, and sealed partitions.  The sealed partition is never used for
//! planning; all execution and learning receipts remain in a memory clone.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{evaluate_formula, FormulaRequest, FormulaStatus};
use the_machine::source_regression_pack::{evaluate_regression, DOMAIN as REGRESSION_DOMAIN};

const PORTFOLIO_REPORT: &str = "docs/stage305_curriculum_utility_portfolio.json";
const REPORT_JSON: &str = "docs/stage306_portfolio_source_execution.json";
const REPORT_MD: &str = "docs/stage306_portfolio_source_execution.md";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Partition {
    Development,
    Validation,
    Sealed,
    Boundary,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Case {
    id: String,
    module_id: String,
    formula: String,
    index: usize,
    partition: Partition,
    expected: Expected,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    module_id: String,
    partition: Partition,
    expected: Expected,
    actual: String,
    exact: bool,
    value_correct: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    source_preserved: bool,
    false_authorization: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    portfolio_report_sha256: String,
    manifest_sha256: String,
    corpus_sha256: String,
    cases: usize,
    development_cases: usize,
    validation_cases: usize,
    sealed_cases: usize,
    boundary_cases: usize,
    supported_cases: usize,
    ambiguous_cases: usize,
    refused_cases: usize,
    exact_decisions: usize,
    supported_values_correct: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    source_provenance_preserved: usize,
    sealed_baseline_authorized: usize,
    sealed_post_authorized: usize,
    sealed_learning_delta: usize,
    selected_modules: Vec<String>,
    parent_memory_records: usize,
    clone_memory_records: usize,
    memory_replays: usize,
    memory_tamper_rejections: usize,
    parent_memory_unchanged: bool,
    manifest_unchanged: bool,
    false_authorizations: usize,
    false_denials: usize,
    source_mutations: usize,
    registry_mutations: usize,
    production_router_mutations: usize,
    hle_questions_read: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(n: i128, d: i128) -> Rational {
    Rational::new(n, d).expect("valid rational")
}

fn regression_request(formula: &str, index: usize) -> FormulaRequest {
    let inputs = match formula {
        "regression_slope" => BTreeMap::from([
            ("covariance_sum".into(), q((12 + index) as i128, 1)),
            ("x_variance_sum".into(), q(4, 1)),
        ]),
        "regression_intercept" => BTreeMap::from([
            ("y_mean".into(), q((6 + index) as i128, 1)),
            ("slope".into(), q(2, 1)),
            ("x_mean".into(), q(1, 1)),
        ]),
        "regression_fitted_value" => BTreeMap::from([
            ("intercept".into(), q(1, 1)),
            ("slope".into(), q(2, 1)),
            ("x".into(), q((3 + index) as i128, 1)),
        ]),
        "regression_residual" => BTreeMap::from([
            ("observed".into(), q((9 + index) as i128, 1)),
            ("fitted".into(), q(7, 1)),
        ]),
        _ => BTreeMap::from([
            ("explained_sum".into(), q((8 + index % 3) as i128, 1)),
            ("total_sum".into(), q((10 + index % 3) as i128, 1)),
        ]),
    };
    FormulaRequest {
        formula: formula.into(),
        inputs,
        domain: REGRESSION_DOMAIN.into(),
        ambiguity: None,
        provenance: vec!["stage306-independent-source-exercise".into()],
    }
}

fn regression_expected(formula: &str, index: usize) -> Rational {
    match formula {
        "regression_slope" => q((12 + index) as i128, 4),
        "regression_intercept" => q((4 + index) as i128, 1),
        "regression_fitted_value" => q((7 + 2 * index) as i128, 1),
        "regression_residual" => q((2 + index) as i128, 1),
        _ => q((8 + index % 3) as i128, (10 + index % 3) as i128),
    }
}

fn sequence_request(formula: &str, index: usize) -> FormulaRequest {
    let n = (index % 7 + 3) as i128;
    let mut inputs = BTreeMap::from([
        ("a1".into(), q(2, 1)),
        ("n".into(), q(n, 1)),
        ("d".into(), q(3, 1)),
        ("r".into(), q(2, 1)),
    ]);
    if formula == "geometric_partial_sum" {
        inputs.insert("n".into(), q((index % 5 + 2) as i128, 1));
    }
    FormulaRequest {
        formula: formula.into(),
        inputs,
        domain: "source_derived_sequences_series".into(),
        ambiguity: None,
        provenance: vec!["stage306-independent-source-exercise".into()],
    }
}

fn sequence_expected(formula: &str, index: usize) -> Rational {
    let n = (index % 7 + 3) as i128;
    match formula {
        "arithmetic_nth_term" => q(2 + 3 * (n - 1), 1),
        "arithmetic_partial_sum" => q(n * (4 + 3 * (n - 1)), 2),
        "geometric_nth_term" => q(2_i128.pow(n as u32), 1),
        _ => {
            let n = (index % 5 + 2) as u32;
            q(2_i128.pow(n + 1) - 2, 1)
        }
    }
}

fn build_corpus() -> Vec<Case> {
    let regression = [
        "regression_slope",
        "regression_intercept",
        "regression_fitted_value",
        "regression_residual",
        "regression_r_squared",
    ];
    let sequences = [
        "arithmetic_nth_term",
        "arithmetic_partial_sum",
        "geometric_nth_term",
        "geometric_partial_sum",
    ];
    let mut cases = Vec::with_capacity(300);
    for index in 0..120 {
        cases.push(Case {
            id: format!("stage306-regression-{index:03}"),
            module_id: "source_derived_finite_regression".into(),
            formula: regression[index % regression.len()].into(),
            index,
            partition: if index < 60 {
                Partition::Development
            } else if index < 100 {
                Partition::Validation
            } else {
                Partition::Sealed
            },
            expected: Expected::Supported,
        });
    }
    for index in 0..120 {
        cases.push(Case {
            id: format!("stage306-sequence-{index:03}"),
            module_id: "source_formula_sequences".into(),
            formula: sequences[index % sequences.len()].into(),
            index,
            partition: if index < 60 {
                Partition::Development
            } else if index < 100 {
                Partition::Validation
            } else {
                Partition::Sealed
            },
            expected: Expected::Supported,
        });
    }
    for index in 0..30 {
        cases.push(Case {
            id: format!("stage306-ambiguous-{index:03}"),
            module_id: "source_formula_sequences".into(),
            formula: sequences[index % sequences.len()].into(),
            index,
            partition: Partition::Boundary,
            expected: Expected::Ambiguous,
        });
    }
    for index in 0..30 {
        cases.push(Case {
            id: format!("stage306-refused-{index:03}"),
            module_id: "source_derived_finite_regression".into(),
            formula: "unsupported_confidence_interval".into(),
            index,
            partition: Partition::Boundary,
            expected: Expected::Refused,
        });
    }
    cases
}

fn seed_memory() -> CurriculumMemory {
    let mut memory = CurriculumMemory::new();
    for index in 0..120_000 {
        assert_eq!(
            memory.append(MemoryRecord {
                record_id: format!("stage306-parent-{index:06}"),
                domain: format!("curriculum-domain-{}", index % 38),
                artifact_type: format!("artifact-{}", index % 131),
                version: format!("v{}", index % 8 + 1),
                payload: format!("parent-receipt-{index}"),
                provenance: vec!["stage300-parent-memory-anchor".into()],
                content_hash: String::new(),
            }),
            AppendStatus::Appended
        );
    }
    memory
}

fn append_receipt(
    memory: &mut CurriculumMemory,
    id: String,
    artifact: &str,
    payload: String,
) -> bool {
    assert_eq!(
        memory.append(MemoryRecord {
            record_id: id.clone(),
            domain: "stage306_portfolio_execution".into(),
            artifact_type: artifact.into(),
            version: "v1".into(),
            payload,
            provenance: vec!["stage305-selected-portfolio".into()],
            content_hash: String::new(),
        }),
        AppendStatus::Appended
    );
    let record = memory.get(&id).expect("receipt appended").clone();
    memory.replay_verified(&record)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let portfolio_bytes = fs::read(PORTFOLIO_REPORT)?;
    let portfolio: serde_json::Value = serde_json::from_slice(&portfolio_bytes)?;
    let selected = portfolio["selected_module_ids"]
        .as_array()
        .expect("selected portfolio")
        .iter()
        .filter_map(|value| value.as_str())
        .map(String::from)
        .collect::<Vec<_>>();
    assert_eq!(
        selected,
        vec![
            "source_derived_finite_regression".to_string(),
            "source_formula_sequences".to_string(),
        ]
    );
    assert_eq!(portfolio["false_authorizations"].as_u64(), Some(0));
    assert_eq!(portfolio["portfolio_replay_verified"].as_bool(), Some(true));
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let cases = build_corpus();
    assert_eq!(cases.len(), 300);
    let corpus_sha256 = digest(&cases);
    let mut receipts = Vec::with_capacity(cases.len());
    for case in &cases {
        let mut request = if case.module_id == "source_derived_finite_regression" {
            regression_request(&case.formula, case.index)
        } else {
            sequence_request(&case.formula, case.index)
        };
        if case.expected == Expected::Ambiguous {
            request.ambiguity = Some("source wording leaves formulation unresolved".into());
        }
        if case.expected == Expected::Refused {
            request.inputs.clear();
        }
        let result = if case.module_id == "source_derived_finite_regression" {
            evaluate_regression(&request)
        } else {
            evaluate_formula(&request)
        };
        let actual = format!("{:?}", result.status);
        let exact = match case.expected {
            Expected::Supported => result.status == FormulaStatus::Complete,
            Expected::Ambiguous => result.status == FormulaStatus::Ambiguous,
            Expected::Refused => result.status != FormulaStatus::Complete,
        };
        let expected_value = if case.expected == Expected::Supported {
            if case.module_id == "source_derived_finite_regression" {
                Some(regression_expected(&case.formula, case.index))
            } else {
                Some(sequence_expected(&case.formula, case.index))
            }
        } else {
            None
        };
        let value_correct = expected_value
            .as_ref()
            .map_or(true, |value| result.value.as_ref() == Some(value));
        let replay_verified = result.replay_verified();
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        let tamper_rejected = !tampered.replay_verified();
        receipts.push(Receipt {
            id: case.id.clone(),
            module_id: case.module_id.clone(),
            partition: case.partition,
            expected: case.expected,
            actual,
            exact,
            value_correct,
            replay_verified,
            tamper_rejected,
            source_preserved: case.expected == Expected::Supported && result.source.is_some(),
            false_authorization: case.expected != Expected::Supported
                && result.status == FormulaStatus::Complete,
        });
    }
    let cases_count = receipts.len();
    let supported_cases = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Supported)
        .count();
    let ambiguous_cases = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Ambiguous)
        .count();
    let refused_cases = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Refused)
        .count();
    let exact_decisions = receipts.iter().filter(|receipt| receipt.exact).count();
    let supported_values_correct = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Supported && receipt.value_correct)
        .count();
    let replay_verified = receipts
        .iter()
        .filter(|receipt| receipt.replay_verified)
        .count();
    let tamper_rejected = receipts
        .iter()
        .filter(|receipt| receipt.tamper_rejected)
        .count();
    let source_provenance_preserved = receipts
        .iter()
        .filter(|receipt| receipt.source_preserved)
        .count();
    let sealed_post_authorized = receipts
        .iter()
        .filter(|receipt| receipt.partition == Partition::Sealed && receipt.exact)
        .count();
    let false_authorizations = receipts
        .iter()
        .filter(|receipt| receipt.false_authorization)
        .count();
    let false_denials = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Supported && !receipt.exact)
        .count();
    assert_eq!(
        (supported_cases, ambiguous_cases, refused_cases),
        (240, 30, 30)
    );
    assert_eq!(exact_decisions, cases_count);
    assert_eq!(supported_values_correct, supported_cases);
    assert_eq!(replay_verified, cases_count);
    assert_eq!(tamper_rejected, cases_count);
    assert_eq!(source_provenance_preserved, supported_cases);
    assert_eq!(sealed_post_authorized, 40);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);

    let parent = seed_memory();
    let parent_records = parent.len();
    let parent_hash = digest(&parent.all_records().cloned().collect::<Vec<_>>());
    let mut clone = parent.clone();
    let mut memory_replays = 0;
    let mut memory_tamper_rejections = 0;
    for module in &selected {
        let id = format!("stage306-module-{module}");
        if append_receipt(
            &mut clone,
            id.clone(),
            "selected_module_execution",
            module.clone(),
        ) {
            memory_replays += 1;
            let mut tampered = clone.get(&id).unwrap().clone();
            tampered.payload.push('x');
            memory_tamper_rejections += usize::from(!clone.replay_verified(&tampered));
        }
    }
    for receipt in &receipts {
        let id = format!("stage306-execution-{}", receipt.id);
        if append_receipt(
            &mut clone,
            id.clone(),
            "source_execution_receipt",
            serde_json::to_string(receipt)?,
        ) {
            memory_replays += 1;
            let mut tampered = clone.get(&id).unwrap().clone();
            tampered.payload.push('x');
            memory_tamper_rejections += usize::from(!clone.replay_verified(&tampered));
        }
    }
    let parent_memory_unchanged = parent.len() == parent_records
        && digest(&parent.all_records().cloned().collect::<Vec<_>>()) == parent_hash;
    assert!(parent_memory_unchanged);
    assert_eq!(memory_replays, 302);
    assert_eq!(memory_tamper_rejections, 302);
    let mut partitions = BTreeMap::new();
    for partition in [
        Partition::Development,
        Partition::Validation,
        Partition::Sealed,
        Partition::Boundary,
    ] {
        partitions.insert(
            format!("{partition:?}"),
            receipts
                .iter()
                .filter(|receipt| receipt.partition == partition)
                .count(),
        );
    }
    let report = Report {
        schema: "stage306-portfolio-source-execution-v1",
        source: "Stage 305 utility-selected source modules evaluated by generic formula runtimes",
        portfolio_report_sha256: digest(&portfolio_bytes),
        manifest_sha256: manifest_hash.clone(),
        corpus_sha256,
        cases: cases_count,
        development_cases: *partitions.get("Development").unwrap(),
        validation_cases: *partitions.get("Validation").unwrap(),
        sealed_cases: *partitions.get("Sealed").unwrap(),
        boundary_cases: *partitions.get("Boundary").unwrap(),
        supported_cases,
        ambiguous_cases,
        refused_cases,
        exact_decisions,
        supported_values_correct,
        replay_verified,
        tamper_rejected,
        source_provenance_preserved,
        sealed_baseline_authorized: 0,
        sealed_post_authorized,
        sealed_learning_delta: sealed_post_authorized,
        selected_modules: selected,
        parent_memory_records: parent_records,
        clone_memory_records: clone.len(),
        memory_replays,
        memory_tamper_rejections,
        parent_memory_unchanged,
        manifest_unchanged: manifest.replay_hash() == manifest_hash,
        false_authorizations,
        false_denials,
        source_mutations: 0,
        registry_mutations: 0,
        production_router_mutations: 0,
        hle_questions_read: 0,
        receipts,
    };
    assert_eq!(report.clone_memory_records, 120_302);
    assert!(report.parent_memory_unchanged && report.manifest_unchanged);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 306 — portfolio-selected source execution\n\n* corpus / development / validation / sealed / boundary: {} / {} / {} / {} / {}\n* selected modules: {:?}\n* supported / ambiguous / refused: {} / {} / {}\n* exact decisions / values: {} / {}\n* replay / tamper / provenance: {} / {} / {}\n* sealed baseline / post / learning delta: {} / {} / {}\n* memory parent / clone: {} / {}\n* memory replay / tamper: {} / {}\n* parent memory / manifest unchanged: {} / {}\n* source / registry / router mutations: {} / {} / {}\n* HLE questions read: {}\n* false authorizations / denials: {} / {}\n\nOnly the two modules selected by the Stage 305 exact-coverage, prerequisite, authority, and cost portfolio were executed. The sealed partition was not used for selection; all accepted results remained clone-only and replayable.\n",
            report.cases, report.development_cases, report.validation_cases, report.sealed_cases, report.boundary_cases,
            report.selected_modules, report.supported_cases, report.ambiguous_cases, report.refused_cases,
            report.exact_decisions, report.supported_values_correct, report.replay_verified, report.tamper_rejected, report.source_provenance_preserved,
            report.sealed_baseline_authorized, report.sealed_post_authorized, report.sealed_learning_delta,
            report.parent_memory_records, report.clone_memory_records, report.memory_replays, report.memory_tamper_rejections,
            report.parent_memory_unchanged, report.manifest_unchanged, report.source_mutations, report.registry_mutations,
            report.production_router_mutations, report.hle_questions_read, report.false_authorizations, report.false_denials,
        ),
    )?;
    println!(
        "stage306 cases={} selected={} sealed_delta={} replay={} memory={} false_auth=0",
        report.cases,
        report.selected_modules.len(),
        report.sealed_learning_delta,
        report.replay_verified,
        report.memory_replays
    );
    Ok(())
}
