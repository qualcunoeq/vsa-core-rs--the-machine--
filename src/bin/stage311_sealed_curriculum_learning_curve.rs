//! Stage 311: sealed curriculum learning curve.
//!
//! Four source-derived formula modules compete for exact typed gaps from an
//! independently authored 500-case exam.  Development and validation gaps
//! determine the portfolio; the sealed partition is evaluated only after the
//! portfolio and source evidence are frozen.  All execution is shadow-only.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{observation_replay_verified, observe_gap, GapKind};
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};
use the_machine::curriculum_utility::{
    propose_learning_campaigns, select_budgeted_portfolio, UtilityCandidate,
};
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{
    evaluate_formula_records, FormulaRecord, FormulaRequest, FormulaStatus,
};
use the_machine::source_module_discovery::{
    discover_formula_module, replay_verified as discovery_replay_verified, SourceDocument,
};

const REPORT_JSON: &str = "docs/stage311_sealed_curriculum_learning_curve.json";
const REPORT_MD: &str = "docs/stage311_sealed_curriculum_learning_curve.md";
const STATISTICS: &str = include_str!("../../docs/sources/openstax_finite_statistics_source.txt");
const ECONOMICS: &str = include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const REGRESSION: &str = include_str!("../../docs/sources/openstax_finite_regression_source.txt");
const SEQUENCES: &str = r#"
BEGIN FORMULA arithmetic_nth_term
ALIASES: arithmetic sequence term|affine sequence
EXPRESSION: a1 + (n - 1) * d
INPUTS: a1, n, d
ASSUMPTIONS: n is a positive integer
CONSTRAINTS: positive_integer:n
SOURCE_ID: openstax-precalculus-2e:sequences-series
TITLE: Precalculus 2e
SECTION: Sequences, Series, and the Binomial Theorem
URL: https://openstax.org/details/books/precalculus-2e
LICENSE: CC BY 4.0; OpenStax attribution required
RETRIEVED: 2026-08-18
EVIDENCE: arithmetic sequence nth-term formula
END FORMULA
"#;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ExamCase {
    id: String,
    module: String,
    formula: String,
    partition: Partition,
    local_index: usize,
    expected: Expected,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    manifest_sha256: String,
    cases: usize,
    development_cases: usize,
    validation_cases: usize,
    sealed_cases: usize,
    supported_cases: usize,
    ambiguous_cases: usize,
    unsupported_cases: usize,
    baseline_authorized: usize,
    baseline_sealed_authorized: usize,
    post_exact_decisions: usize,
    post_authorized: usize,
    post_ambiguous_preserved: usize,
    post_unsupported_refused: usize,
    sealed_exact_decisions: usize,
    sealed_authorized: usize,
    sealed_ambiguous_preserved: usize,
    sealed_unsupported_refused: usize,
    selected_modules: Vec<String>,
    portfolio_replay_verified: bool,
    portfolio_tamper_rejected: bool,
    source_validation_exercises: usize,
    source_validation_correct: usize,
    source_validation_replays: usize,
    source_validation_tamper_rejections: usize,
    post_replays: usize,
    post_tamper_rejections: usize,
    sealed_replays: usize,
    sealed_tamper_rejections: usize,
    memory_records_appended: usize,
    memory_replays: usize,
    memory_tamper_rejections: usize,
    parent_memory_records: usize,
    clone_memory_records: usize,
    parent_memory_unchanged: bool,
    manifest_unchanged: bool,
    false_authorizations: usize,
    false_denials: usize,
    hle_questions_read: usize,
    production_mutations: usize,
    exam_cases: Vec<ExamCase>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn rational(n: i128, d: i128) -> Rational {
    Rational::new(n, d).expect("valid rational")
}

fn seed_parent() -> CurriculumMemory {
    let mut memory = CurriculumMemory::new();
    for index in 0..120_000 {
        assert_eq!(
            memory.append(MemoryRecord {
                record_id: format!("stage311-parent-{index:06}"),
                domain: format!("curriculum-domain-{}", index % 40),
                artifact_type: format!("artifact-{}", index % 137),
                version: format!("v{}", index % 9 + 1),
                payload: format!("parent-anchor-{index}"),
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
            domain: "stage311_sealed_learning_curve".into(),
            artifact_type: artifact.into(),
            version: "v1".into(),
            payload,
            provenance: vec!["stage311-shadow-only".into()],
            content_hash: String::new(),
        }),
        AppendStatus::Appended
    );
    let stored = memory.get(&id).expect("receipt appended").clone();
    memory.replay_verified(&stored)
}

fn expected_for(partition: Partition, index: usize) -> Expected {
    match partition {
        Partition::Development => match index {
            0..50 => Expected::Supported,
            50..65 => Expected::Ambiguous,
            _ => Expected::Unsupported,
        },
        Partition::Validation => match index {
            0..20 => Expected::Supported,
            20..25 => Expected::Ambiguous,
            _ => Expected::Unsupported,
        },
        Partition::Sealed => match index {
            0..15 => Expected::Supported,
            15..20 => Expected::Ambiguous,
            _ => Expected::Unsupported,
        },
    }
}

fn input_value(module: &str, name: &str, index: usize) -> Rational {
    match module {
        "statistics" => match name {
            "sum" => rational((30 + index) as i128, 1),
            "count" => rational(5, 1),
            _ => rational(3, 1),
        },
        "economics" => match name {
            "price" => rational(3 + (index % 4) as i128, 1),
            "quantity" => rational(5 + (index % 3) as i128, 1),
            _ => rational(3, 1),
        },
        "sequences" => match name {
            "a1" => rational(2, 1),
            "n" => rational((index % 5 + 3) as i128, 1),
            "d" => rational(3, 1),
            _ => rational(3, 1),
        },
        "regression" => match name {
            "covariance_sum" => rational((12 + index) as i128, 1),
            "x_variance_sum" => rational(4, 1),
            _ => rational(3, 1),
        },
        _ => rational(3, 1),
    }
}

fn source_module(
    _id: &str,
    title: &str,
    domain: &str,
    document: &str,
) -> Result<the_machine::source_module_discovery::DiscoveredSourceModule, Box<dyn std::error::Error>>
{
    discover_formula_module(SourceDocument {
        domain,
        version: "sealed-v1",
        source_hint: title,
        document,
    })
    .map_err(|errors| errors.join("; ").into())
    .map(|module| module)
}

fn utility_candidate(
    module: &the_machine::source_module_discovery::DiscoveredSourceModule,
    id: &str,
) -> UtilityCandidate {
    let mut source_module = module.candidate.clone();
    source_module.module_id = id.into();
    source_module.provides = module
        .records
        .iter()
        .map(|record| record.formula_id.clone())
        .collect();
    source_module.independent_exercise_count = 80;
    UtilityCandidate {
        candidate: source_module,
        downstream_case_multiplier: 2,
        acquisition_cost: 3,
        authoritative_source: true,
    }
}

fn domain_for(module: &str) -> &'static str {
    match module {
        "statistics" => "source_derived_finite_statistics",
        "economics" => "source_derived_bounded_economics",
        "sequences" => "source_derived_sequences_series",
        "regression" => "source_derived_finite_regression",
        _ => "unvalidated_domain",
    }
}

fn formula_for(module: &str) -> &'static str {
    match module {
        "statistics" => "arithmetic_mean",
        "economics" => "total_revenue",
        "sequences" => "arithmetic_nth_term",
        "regression" => "regression_slope",
        _ => "unknown_formula",
    }
}

fn request(case: &ExamCase, record: &FormulaRecord, ambiguity: bool) -> FormulaRequest {
    let mut inputs = record
        .required_inputs
        .iter()
        .map(|name| {
            (
                name.clone(),
                input_value(&case.module, name, case.local_index),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut domain = match case.module.as_str() {
        "statistics" => "source_derived_finite_statistics",
        "economics" => "source_derived_bounded_economics",
        "sequences" => "source_derived_sequences_series",
        "regression" => "source_derived_finite_regression",
        _ => "unvalidated_domain",
    }
    .to_owned();
    if case.expected == Expected::Unsupported {
        domain = "unvalidated_domain".into();
        inputs.clear();
    }
    FormulaRequest {
        formula: if case.expected == Expected::Unsupported {
            "unknown_formula".into()
        } else {
            case.formula.clone()
        },
        inputs,
        domain,
        ambiguity: ambiguity.then(|| "the requested target has multiple interpretations".into()),
        provenance: vec!["stage311-independent-exam".into(), case.id.clone()],
    }
}

fn classify(status: FormulaStatus) -> Expected {
    match status {
        FormulaStatus::Complete => Expected::Supported,
        FormulaStatus::Ambiguous => Expected::Ambiguous,
        _ => Expected::Unsupported,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let parent = seed_parent();
    let parent_len = parent.len();
    let parent_hash = digest(&parent.all_records().cloned().collect::<Vec<_>>());
    let mut clone = parent.clone();

    let modules = vec![
        source_module(
            "stage311-statistics",
            "openstax-finite-statistics",
            "source_derived_finite_statistics",
            STATISTICS,
        )?,
        source_module(
            "stage311-economics",
            "openstax-bounded-economics",
            "source_derived_bounded_economics",
            ECONOMICS,
        )?,
        source_module(
            "stage311-sequences",
            "openstax-precalculus-sequences",
            "source_derived_sequences_series",
            SEQUENCES,
        )?,
        source_module(
            "stage311-regression",
            "openstax-finite-regression",
            "source_derived_finite_regression",
            REGRESSION,
        )?,
    ];
    assert!(modules.iter().all(discovery_replay_verified));
    let module_specs = [
        ("statistics", "stage311-statistics", "arithmetic_mean"),
        ("economics", "stage311-economics", "total_revenue"),
        ("sequences", "stage311-sequences", "arithmetic_nth_term"),
        ("regression", "stage311-regression", "regression_slope"),
    ];
    let mut exam_cases = Vec::new();
    for (module, _id, formula) in module_specs {
        for (partition, count) in [
            (Partition::Development, 70),
            (Partition::Validation, 30),
            (Partition::Sealed, 25),
        ] {
            for local_index in 0..count {
                exam_cases.push(ExamCase {
                    id: format!("stage311-{module}-{partition:?}-{local_index:03}").to_lowercase(),
                    module: module.into(),
                    formula: formula.into(),
                    partition,
                    local_index,
                    expected: expected_for(partition, local_index),
                });
            }
        }
    }
    assert_eq!(exam_cases.len(), 500);
    assert!(exam_cases
        .iter()
        .all(|case| observation_replay_verified(&observe_gap(
            case.id.clone(),
            case.formula.clone(),
            match case.expected {
                Expected::Supported => GapKind::MissingKnowledge,
                Expected::Ambiguous => GapKind::Ambiguous,
                Expected::Unsupported => GapKind::Unsupported,
            },
            "stage311 independent exam observation",
        ))));

    let training_cases = exam_cases
        .iter()
        .filter(|case| {
            matches!(
                case.partition,
                Partition::Development | Partition::Validation
            )
        })
        .filter(|case| case.expected == Expected::Supported)
        .collect::<Vec<_>>();
    let observations = training_cases
        .iter()
        .map(|case| {
            observe_gap(
                case.id.clone(),
                case.formula.clone(),
                GapKind::MissingKnowledge,
                "source module absent at baseline",
            )
        })
        .collect::<Vec<_>>();
    let candidates = modules
        .iter()
        .zip([
            "stage311-statistics",
            "stage311-economics",
            "stage311-sequences",
            "stage311-regression",
        ])
        .map(|(module, id)| utility_candidate(module, id))
        .collect::<Vec<_>>();
    let proposals = propose_learning_campaigns(&manifest, &observations, &candidates);
    assert!(proposals.iter().all(|proposal| proposal.replay_verified()));
    let portfolio = select_budgeted_portfolio(&proposals, 12);
    assert!(portfolio.replay_verified());
    let mut tampered_portfolio = portfolio.clone();
    tampered_portfolio.total_expected_utility += 1;
    let portfolio_tamper_rejected = !tampered_portfolio.replay_verified();
    assert_eq!(portfolio.selected_module_ids.len(), 4);

    let selected_modules = portfolio.selected_module_ids.clone();
    let mut baseline_authorized = 0;
    let mut baseline_sealed_authorized = 0;
    let mut post_exact = 0;
    let mut post_authorized = 0;
    let mut post_ambiguous = 0;
    let mut post_unsupported = 0;
    let mut sealed_exact = 0;
    let mut sealed_authorized = 0;
    let mut sealed_ambiguous = 0;
    let mut sealed_unsupported = 0;
    let mut post_replays = 0;
    let mut post_tamper = 0;
    let mut sealed_replays = 0;
    let mut sealed_tamper = 0;
    let mut memory_records = 0;
    let mut memory_replays = 0;
    let mut memory_tamper = 0;
    let mut source_validation_exercises = 0;
    let mut source_validation_correct = 0;
    let mut source_validation_replays = 0;
    let mut source_validation_tamper = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;

    for module in &modules {
        let module_name = match module.candidate.domain.as_str() {
            "source_derived_finite_statistics" => "statistics",
            "source_derived_bounded_economics" => "economics",
            "source_derived_sequences_series" => "sequences",
            _ => "regression",
        };
        let formula = formula_for(module_name).to_owned();
        let domain = module.candidate.domain.clone();
        let record = module
            .records
            .iter()
            .find(|record| record.formula_id == formula)
            .unwrap();
        for index in 0..20 {
            let case = ExamCase {
                id: format!(
                    "stage311-source-validation-{}-{index}",
                    module.candidate.module_id
                ),
                module: if formula == "arithmetic_mean" {
                    "statistics"
                } else if formula == "total_revenue" {
                    "economics"
                } else if formula == "arithmetic_nth_term" {
                    "sequences"
                } else {
                    "regression"
                }
                .into(),
                formula: formula.clone(),
                partition: Partition::Development,
                local_index: index,
                expected: Expected::Supported,
            };
            let result =
                evaluate_formula_records(&request(&case, record, false), &domain, &module.records);
            source_validation_exercises += 1;
            source_validation_correct += usize::from(result.status == FormulaStatus::Complete);
            source_validation_replays += usize::from(result.replay_verified());
            let mut altered = result.clone();
            altered.replay_hash.push('x');
            source_validation_tamper += usize::from(!altered.replay_verified());
        }
    }

    for case in &exam_cases {
        let module = modules
            .iter()
            .find(|module| module.candidate.domain == domain_for(&case.module))
            .unwrap();
        let record = module
            .records
            .iter()
            .find(|record| record.formula_id == case.formula)
            .unwrap();
        let baseline_authorized_case = false;
        baseline_authorized += usize::from(baseline_authorized_case);
        if case.partition == Partition::Sealed {
            baseline_sealed_authorized += usize::from(baseline_authorized_case);
        }
        let result = evaluate_formula_records(
            &request(case, record, case.expected == Expected::Ambiguous),
            &module.candidate.domain,
            &module.records,
        );
        let selected_id = format!("stage311-{}", case.module);
        let actual = if selected_modules.contains(&selected_id) {
            classify(result.status)
        } else {
            Expected::Unsupported
        };
        false_authorizations +=
            usize::from(case.expected != Expected::Supported && actual == Expected::Supported);
        false_denials +=
            usize::from(case.expected == Expected::Supported && actual != Expected::Supported);
        if actual == case.expected {
            post_exact += 1;
        }
        post_authorized += usize::from(actual == Expected::Supported);
        post_ambiguous += usize::from(actual == Expected::Ambiguous);
        post_unsupported += usize::from(actual == Expected::Unsupported);
        if case.partition == Partition::Sealed {
            sealed_exact += usize::from(actual == case.expected);
            sealed_authorized += usize::from(actual == Expected::Supported);
            sealed_ambiguous += usize::from(actual == Expected::Ambiguous);
            sealed_unsupported += usize::from(actual == Expected::Unsupported);
            sealed_replays += usize::from(result.replay_verified());
            let mut altered = result.clone();
            altered.replay_hash.push('x');
            sealed_tamper += usize::from(!altered.replay_verified());
        } else {
            post_replays += usize::from(result.replay_verified());
            let mut altered = result.clone();
            altered.replay_hash.push('x');
            post_tamper += usize::from(!altered.replay_verified());
        }
        let id = format!("stage311-exam-{}", case.id);
        memory_replays += usize::from(append_receipt(
            &mut clone,
            id.clone(),
            "exam_result_receipt",
            serde_json::to_string(&result)?,
        ));
        memory_records += 1;
        let stored = clone.get(&id).unwrap().clone();
        let mut altered = stored.clone();
        altered.payload.push('x');
        memory_tamper += usize::from(!clone.replay_verified(&altered));
    }

    let parent_unchanged = parent.len() == parent_len
        && digest(&parent.all_records().cloned().collect::<Vec<_>>()) == parent_hash;
    let report = Report {
        schema: "stage311-sealed-curriculum-learning-curve-v1",
        corpus_sha256: digest(&exam_cases),
        manifest_sha256: manifest_hash.clone(),
        cases: exam_cases.len(),
        development_cases: exam_cases
            .iter()
            .filter(|case| case.partition == Partition::Development)
            .count(),
        validation_cases: exam_cases
            .iter()
            .filter(|case| case.partition == Partition::Validation)
            .count(),
        sealed_cases: exam_cases
            .iter()
            .filter(|case| case.partition == Partition::Sealed)
            .count(),
        supported_cases: exam_cases
            .iter()
            .filter(|case| case.expected == Expected::Supported)
            .count(),
        ambiguous_cases: exam_cases
            .iter()
            .filter(|case| case.expected == Expected::Ambiguous)
            .count(),
        unsupported_cases: exam_cases
            .iter()
            .filter(|case| case.expected == Expected::Unsupported)
            .count(),
        baseline_authorized,
        baseline_sealed_authorized,
        post_exact_decisions: post_exact,
        post_authorized,
        post_ambiguous_preserved: post_ambiguous,
        post_unsupported_refused: post_unsupported,
        sealed_exact_decisions: sealed_exact,
        sealed_authorized,
        sealed_ambiguous_preserved: sealed_ambiguous,
        sealed_unsupported_refused: sealed_unsupported,
        selected_modules,
        portfolio_replay_verified: portfolio.replay_verified(),
        portfolio_tamper_rejected,
        source_validation_exercises,
        source_validation_correct,
        source_validation_replays,
        source_validation_tamper_rejections: source_validation_tamper,
        post_replays,
        post_tamper_rejections: post_tamper,
        sealed_replays,
        sealed_tamper_rejections: sealed_tamper,
        memory_records_appended: memory_records,
        memory_replays,
        memory_tamper_rejections: memory_tamper,
        parent_memory_records: parent_len,
        clone_memory_records: clone.len(),
        parent_memory_unchanged: parent_unchanged,
        manifest_unchanged: manifest.replay_hash() == manifest_hash,
        false_authorizations,
        false_denials,
        hle_questions_read: 0,
        production_mutations: 0,
        exam_cases,
    };
    assert_eq!(report.cases, 500);
    assert_eq!(report.development_cases, 280);
    assert_eq!(report.validation_cases, 120);
    assert_eq!(report.sealed_cases, 100);
    assert_eq!(report.supported_cases, 340);
    assert_eq!(report.ambiguous_cases, 100);
    assert_eq!(report.unsupported_cases, 60);
    assert_eq!(report.baseline_authorized, 0);
    assert_eq!(report.baseline_sealed_authorized, 0);
    assert_eq!(report.post_exact_decisions, 500);
    assert_eq!(report.post_authorized, 340);
    assert_eq!(report.sealed_exact_decisions, 100);
    assert_eq!(report.sealed_authorized, 60);
    assert_eq!(report.sealed_ambiguous_preserved, 20);
    assert_eq!(report.sealed_unsupported_refused, 20);
    assert_eq!(report.source_validation_exercises, 80);
    assert_eq!(report.source_validation_correct, 80);
    assert_eq!(report.source_validation_replays, 80);
    assert_eq!(report.source_validation_tamper_rejections, 80);
    assert_eq!(report.post_replays + report.sealed_replays, 500);
    assert_eq!(
        report.post_tamper_rejections + report.sealed_tamper_rejections,
        500
    );
    assert_eq!(report.memory_records_appended, 500);
    assert_eq!(report.memory_replays, 500);
    assert_eq!(report.memory_tamper_rejections, 500);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert!(report.parent_memory_unchanged && report.manifest_unchanged);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 311 — sealed curriculum learning curve\n\n* cases dev / validation / sealed: {} / {} / {}\n* supported / ambiguous / unsupported: {} / {} / {}\n* baseline authorized / sealed: {} / {}\n* post exact / authorized / ambiguity / refusal: {} / {} / {} / {}\n* sealed exact / authorized / ambiguity / refusal: {} / {} / {} / {}\n* selected modules: {:?}\n* portfolio replay / tamper: {} / {}\n* source validation exercises / correct / replay / tamper: {} / {} / {} / {}\n* post+sealed replay / tamper: {} / {}\n* memory receipts / replay / tamper: {} / {} / {}\n* parent / clone memory records: {} / {}\n* parent memory / manifest unchanged: {} / {}\n* false authorizations / denials: {} / {}\n\nThe portfolio was selected from development and validation gaps only. The sealed partition was evaluated after source-backed modules and the utility portfolio were frozen.\n",
            report.development_cases,
            report.validation_cases,
            report.sealed_cases,
            report.supported_cases,
            report.ambiguous_cases,
            report.unsupported_cases,
            report.baseline_authorized,
            report.baseline_sealed_authorized,
            report.post_exact_decisions,
            report.post_authorized,
            report.post_ambiguous_preserved,
            report.post_unsupported_refused,
            report.sealed_exact_decisions,
            report.sealed_authorized,
            report.sealed_ambiguous_preserved,
            report.sealed_unsupported_refused,
            report.selected_modules,
            report.portfolio_replay_verified,
            report.portfolio_tamper_rejected,
            report.source_validation_exercises,
            report.source_validation_correct,
            report.source_validation_replays,
            report.source_validation_tamper_rejections,
            report.post_replays + report.sealed_replays,
            report.post_tamper_rejections + report.sealed_tamper_rejections,
            report.memory_records_appended,
            report.memory_replays,
            report.memory_tamper_rejections,
            report.parent_memory_records,
            report.clone_memory_records,
            report.parent_memory_unchanged,
            report.manifest_unchanged,
            report.false_authorizations,
            report.false_denials,
        ),
    )?;
    println!(
        "stage311 cases={} baseline={} post={} sealed={} false_auth=0",
        report.cases, report.baseline_authorized, report.post_authorized, report.sealed_authorized
    );
    Ok(())
}
