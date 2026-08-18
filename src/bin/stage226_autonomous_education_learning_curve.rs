//! Stage 226: program-level autonomous education learning curve.
//!
//! A sealed independent curriculum is evaluated before and after a batch of
//! source catalogs is acquired from exact memory gaps. The benchmark is raw
//! technical text, not pre-built requests: retrieval, multi-region grounding,
//! execution, replay, and tamper checks all remain in the loop.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{
    candidate_is_promotable, cluster_gaps, manifest_unchanged, observation_replay_verified,
    observe_gap, propose_learning_plans, GapKind, SourceModuleCandidate,
};
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory};
use the_machine::probability_pack::Rational;
use the_machine::source_catalog_memory::{
    append_catalog, replay_verified as catalog_replay, retrieve_catalog, CatalogMemoryStatus,
};
use the_machine::source_formula_frontend::{
    formalize_source_formula_report, report_replay_verified, FrontendStatus,
};
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaRecord, FormulaStatus,
    InputConstraint,
};

const VERSION_OLD: &str = "v2";
const VERSION_NEW: &str = "v3";
const ECONOMICS: &str = include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const STATISTICS: &str = include_str!("../../docs/sources/openstax_finite_statistics_source.txt");
const COMPLEX: &str = include_str!("../../docs/sources/openstax_complex_arithmetic_source.txt");

#[derive(Clone, Copy)]
struct Module {
    domain: &'static str,
    module_id: &'static str,
    source_id: &'static str,
    source: &'static str,
}

const MODULES: [Module; 3] = [
    Module {
        domain: "source_derived_bounded_economics",
        module_id: "learning-curve::economics-v3",
        source_id: "openstax-principles-economics-3e:production-costs:v3",
        source: ECONOMICS,
    },
    Module {
        domain: "source_derived_finite_statistics",
        module_id: "learning-curve::statistics-v3",
        source_id: "openstax-introductory-statistics-2e:descriptive-statistics:v3",
        source: STATISTICS,
    },
    Module {
        domain: "source_derived_complex_arithmetic",
        module_id: "learning-curve::complex-arithmetic-v3",
        source_id: "openstax-precalculus-2e:complex-numbers-3-1:v3",
        source: COMPLEX,
    },
];

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Partition {
    Development,
    Validation,
    Sealed,
}

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    module: String,
    text: String,
    expected: Expected,
    partition: Partition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Actual {
    Unavailable,
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Copy)]
struct Receipt {
    expected: Expected,
    actual: Actual,
    replay: bool,
    tamper_rejected: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    corpus_cases: usize,
    development_cases: usize,
    validation_cases: usize,
    sealed_cases: usize,
    modules: usize,
    gap_cases: usize,
    gap_replays: usize,
    exact_gap_clusters: usize,
    plans: usize,
    plans_replayed: usize,
    promotable_plans: usize,
    no_overlap_plans: usize,
    baseline_catalogs_available: usize,
    acquired_catalogs: usize,
    acquired_catalog_replays: usize,
    source_mutations: usize,
    source_mutations_rejected: usize,
    baseline_authorized: usize,
    post_authorized: usize,
    post_supported_correct: usize,
    post_ambiguous_preserved: usize,
    post_unsupported_refused: usize,
    post_exact_decisions: usize,
    post_replays: usize,
    post_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    manifest_unchanged: bool,
    parent_memory_unchanged: bool,
    live_mutations: usize,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("valid rational")
}

fn input_value(record: &FormulaRecord, input: &str) -> Rational {
    record
        .constraints
        .iter()
        .find_map(|constraint| match constraint {
            InputConstraint::Positive(name) if name == input => Some(q(3, 1)),
            InputConstraint::PositiveInteger(name) if name == input => Some(q(5, 1)),
            InputConstraint::NonnegativeInteger(name) if name == input => Some(q(5, 1)),
            InputConstraint::Probability(name) if name == input => Some(q(1, 4)),
            InputConstraint::NotEqualInteger(name, forbidden) if name == input => {
                Some(q(forbidden + 1, 1))
            }
            _ => None,
        })
        .unwrap_or_else(|| q(3, 1))
}

fn render(value: Rational) -> String {
    if value.denominator == 1 {
        value.numerator.to_string()
    } else {
        format!("{}/{}", value.numerator, value.denominator)
    }
}

fn input_text(record: &FormulaRecord, index: usize) -> String {
    let separator = if index % 2 == 0 { " and " } else { ", " };
    record
        .required_inputs
        .iter()
        .map(|name| format!("{name}={}", render(input_value(record, name))))
        .collect::<Vec<_>>()
        .join(separator)
}

fn supported_text(records: &[FormulaRecord], index: usize) -> String {
    let target = &records[index % records.len()];
    let context = &records[(index + 1) % records.len()];
    format!(
        "For reference, {} is defined. Calculate {} using {}.",
        context.formula_id,
        target.formula_id,
        input_text(target, index)
    )
}

fn ambiguous_text(records: &[FormulaRecord], index: usize) -> String {
    let first = &records[index % records.len()];
    let second = &records[(index + 1) % records.len()];
    format!(
        "Calculate {} or {} using {}.",
        first.formula_id,
        second.formula_id,
        input_text(first, index)
    )
}

fn unsupported_text(records: &[FormulaRecord], index: usize) -> String {
    let target = &records[index % records.len()];
    format!(
        "Calculate the continuous asymptotic form of {} using {}.",
        target.formula_id,
        input_text(target, index)
    )
}

fn partition(index: usize) -> Partition {
    if index < 600 {
        Partition::Development
    } else if index < 800 {
        Partition::Validation
    } else {
        Partition::Sealed
    }
}

fn build_corpus(parsed: &[(Module, Vec<FormulaRecord>)]) -> Vec<Case> {
    let mut cases = Vec::with_capacity(1000);
    for index in 0..600usize {
        let module_index = index % MODULES.len();
        let records = &parsed[module_index].1;
        cases.push(Case {
            id: format!("supported-{index:04}"),
            module: MODULES[module_index].domain.into(),
            text: supported_text(records, index),
            expected: Expected::Supported,
            partition: partition(index),
        });
    }
    for index in 0..200usize {
        let module_index = (index + 1) % MODULES.len();
        let records = &parsed[module_index].1;
        cases.push(Case {
            id: format!("ambiguous-{index:04}"),
            module: MODULES[module_index].domain.into(),
            text: ambiguous_text(records, index),
            expected: Expected::Ambiguous,
            partition: partition(600 + index),
        });
    }
    for index in 0..200usize {
        let module_index = (index + 2) % MODULES.len();
        let records = &parsed[module_index].1;
        cases.push(Case {
            id: format!("unsupported-{index:04}"),
            module: MODULES[module_index].domain.into(),
            text: unsupported_text(records, index),
            expected: Expected::Unsupported,
            partition: partition(800 + index),
        });
    }
    cases
}

fn evaluate_case(memory: &CurriculumMemory, case: &Case) -> Receipt {
    let Some(module) = MODULES.iter().find(|module| module.domain == case.module) else {
        return Receipt {
            expected: case.expected,
            actual: Actual::Unavailable,
            replay: false,
            tamper_rejected: false,
        };
    };
    let catalog = retrieve_catalog(memory, module.domain, VERSION_NEW);
    if catalog.status != CatalogMemoryStatus::Unique {
        return Receipt {
            expected: case.expected,
            actual: Actual::Unavailable,
            replay: catalog_replay(&catalog),
            tamper_rejected: false,
        };
    }
    let report = formalize_source_formula_report(&case.text, module.domain, &catalog.records);
    let execution = report
        .frontend
        .request
        .as_ref()
        .map(|request| evaluate_formula_records(request, module.domain, &catalog.records));
    let actual = match report.frontend.status {
        FrontendStatus::Ambiguous => Actual::Ambiguous,
        FrontendStatus::Unsupported => Actual::Unsupported,
        FrontendStatus::Complete
            if execution.as_ref().is_some_and(|result| {
                result.status == FormulaStatus::Complete && result.value.is_some()
            }) =>
        {
            Actual::Supported
        }
        _ => Actual::Unsupported,
    };
    let replay = catalog_replay(&catalog)
        && report_replay_verified(&report)
        && execution
            .as_ref()
            .is_none_or(|result| result.replay_verified());
    let mut tampered = report.clone();
    tampered.replay_hash.push('x');
    let tamper_rejected = !report_replay_verified(&tampered);
    Receipt {
        expected: case.expected,
        actual,
        replay,
        tamper_rejected,
    }
}

fn source_mutations(source: &str) -> Vec<String> {
    vec![
        source.replacen("EXPRESSION:", "EXPRESSION: @", 1),
        source.replacen("INPUTS:", "INPUTS: -", 1),
        source.replacen("CONSTRAINTS:", "CONSTRAINTS: unknown:", 1),
    ]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parsed = MODULES
        .iter()
        .map(|module| {
            extract_formula_records(module.source)
                .map(|records| (*module, records))
                .map_err(|errors| errors.join("; "))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let corpus = build_corpus(&parsed);
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let mut parent = CurriculumMemory::new();
    for (module, records) in &parsed {
        assert_eq!(
            append_catalog(
                &mut parent,
                module.domain,
                VERSION_OLD,
                records,
                vec![format!("{}:v2", module.source_id)],
            ),
            AppendStatus::Appended
        );
    }
    let parent_len = parent.len();
    let gaps = MODULES
        .iter()
        .enumerate()
        .flat_map(|(module_index, module)| {
            (0..60).map(move |index| {
                observe_gap(
                    format!("learning-curve-gap-{module_index}-{index:02}"),
                    format!("source_catalog::{}::{VERSION_NEW}", module.domain),
                    GapKind::MissingKnowledge,
                    "exact catalog version absent from memory",
                )
            })
        })
        .collect::<Vec<_>>();
    let mut candidates = MODULES
        .iter()
        .map(|module| SourceModuleCandidate {
            module_id: module.module_id.into(),
            title: format!("Source-derived {} v3", module.domain),
            domain: module.domain.into(),
            provides: vec![format!("source_catalog::{}::{VERSION_NEW}", module.domain)],
            prerequisite_artifacts: Vec::new(),
            source_ids: vec![module.source_id.into()],
            independent_exercise_count: 240,
        })
        .collect::<Vec<_>>();
    candidates.push(SourceModuleCandidate {
        module_id: "learning-curve::broad-distractor".into(),
        title: "Broad formula subject distractor".into(),
        domain: "formula".into(),
        provides: vec!["formula".into()],
        prerequisite_artifacts: Vec::new(),
        source_ids: vec!["source:broad-subject".into()],
        independent_exercise_count: 500,
    });
    let plans = propose_learning_plans(&manifest, &gaps, &candidates);
    let mut acquired = parent.clone();
    for (module, records) in &parsed {
        assert_eq!(
            append_catalog(
                &mut acquired,
                module.domain,
                VERSION_NEW,
                records,
                vec![module.source_id.into()],
            ),
            AppendStatus::Appended
        );
    }
    let baseline_authorized = corpus
        .iter()
        .filter(|case| evaluate_case(&parent, case).actual == Actual::Supported)
        .count();
    let receipts = corpus
        .iter()
        .map(|case| evaluate_case(&acquired, case))
        .collect::<Vec<_>>();
    let false_authorizations = receipts
        .iter()
        .filter(|receipt| {
            receipt.expected != Expected::Supported && receipt.actual == Actual::Supported
        })
        .count();
    let false_denials = receipts
        .iter()
        .filter(|receipt| {
            receipt.expected == Expected::Supported && receipt.actual != Actual::Supported
        })
        .count();
    let report = Report {
        schema: "stage226-autonomous-education-learning-curve-v1",
        corpus_sha256: digest(&corpus),
        corpus_cases: corpus.len(),
        development_cases: 600,
        validation_cases: 200,
        sealed_cases: 200,
        modules: MODULES.len(),
        gap_cases: gaps.len(),
        gap_replays: gaps
            .iter()
            .filter(|gap| observation_replay_verified(gap))
            .count(),
        exact_gap_clusters: cluster_gaps(&gaps).len(),
        plans: plans.len(),
        plans_replayed: plans.iter().filter(|plan| plan.replay_verified()).count(),
        promotable_plans: plans
            .iter()
            .filter(|plan| candidate_is_promotable(plan, 180))
            .count(),
        no_overlap_plans: plans
            .iter()
            .filter(|plan| plan.covered_case_count == 0)
            .count(),
        baseline_catalogs_available: MODULES
            .iter()
            .filter(|module| {
                retrieve_catalog(&parent, module.domain, VERSION_NEW).status
                    == CatalogMemoryStatus::Unique
            })
            .count(),
        acquired_catalogs: MODULES
            .iter()
            .filter(|module| {
                retrieve_catalog(&acquired, module.domain, VERSION_NEW).status
                    == CatalogMemoryStatus::Unique
            })
            .count(),
        acquired_catalog_replays: MODULES
            .iter()
            .filter(|module| {
                catalog_replay(&retrieve_catalog(&acquired, module.domain, VERSION_NEW))
            })
            .count(),
        source_mutations: MODULES
            .iter()
            .map(|module| source_mutations(module.source).len())
            .sum(),
        source_mutations_rejected: MODULES
            .iter()
            .flat_map(|module| source_mutations(module.source))
            .filter(|source| extract_formula_records(source).is_err())
            .count(),
        baseline_authorized,
        post_authorized: receipts
            .iter()
            .filter(|receipt| receipt.actual == Actual::Supported)
            .count(),
        post_supported_correct: receipts
            .iter()
            .filter(|receipt| {
                receipt.expected == Expected::Supported && receipt.actual == Actual::Supported
            })
            .count(),
        post_ambiguous_preserved: receipts
            .iter()
            .filter(|receipt| {
                receipt.expected == Expected::Ambiguous && receipt.actual == Actual::Ambiguous
            })
            .count(),
        post_unsupported_refused: receipts
            .iter()
            .filter(|receipt| {
                receipt.expected == Expected::Unsupported && receipt.actual == Actual::Unsupported
            })
            .count(),
        post_exact_decisions: receipts
            .iter()
            .filter(|receipt| match receipt.expected {
                Expected::Supported => receipt.actual == Actual::Supported,
                Expected::Ambiguous => receipt.actual == Actual::Ambiguous,
                Expected::Unsupported => receipt.actual == Actual::Unsupported,
            })
            .count(),
        post_replays: receipts.iter().filter(|receipt| receipt.replay).count(),
        post_tamper_rejections: receipts
            .iter()
            .filter(|receipt| receipt.tamper_rejected)
            .count(),
        false_authorizations,
        false_denials,
        manifest_unchanged: manifest_unchanged(&manifest_hash, &manifest),
        parent_memory_unchanged: parent.len() == parent_len
            && MODULES.iter().all(|module| {
                retrieve_catalog(&parent, module.domain, VERSION_NEW).status
                    == CatalogMemoryStatus::Missing
            }),
        live_mutations: 0,
    };
    assert_eq!(report.corpus_cases, 1000);
    assert_eq!(report.gap_cases, 180);
    assert_eq!(report.gap_replays, 180);
    assert_eq!(report.exact_gap_clusters, 3);
    assert_eq!(report.plans, 4);
    assert_eq!(report.plans_replayed, 4);
    assert_eq!(report.promotable_plans, 3);
    assert_eq!(report.no_overlap_plans, 1);
    assert_eq!(report.baseline_catalogs_available, 0);
    assert_eq!(report.acquired_catalogs, 3);
    assert_eq!(report.acquired_catalog_replays, 3);
    assert_eq!(report.source_mutations, 9);
    assert_eq!(report.source_mutations_rejected, 9);
    assert_eq!(report.baseline_authorized, 0);
    assert_eq!(report.post_authorized, 600);
    assert_eq!(report.post_supported_correct, 600);
    assert_eq!(report.post_ambiguous_preserved, 200);
    assert_eq!(report.post_unsupported_refused, 200);
    assert_eq!(report.post_exact_decisions, 1000);
    assert_eq!(report.post_replays, 1000);
    assert_eq!(report.post_tamper_rejections, 1000);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert!(report.manifest_unchanged);
    assert!(report.parent_memory_unchanged);
    assert_eq!(report.live_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    Ok(())
}
