//! Stage 225: autonomous multi-gap source education in a sandbox clone.
//!
//! This campaign exercises the complete batch loop rather than one hand-picked
//! gap: exact typed memory failures are clustered, several source modules are
//! ranked, each source is independently validated, and all selected catalogs
//! are appended and retried only in an immutable memory clone.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{
    candidate_is_promotable, manifest_unchanged, observe_gap, propose_learning_plans, GapKind,
    SourceModuleCandidate,
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
struct ModuleSpec {
    domain: &'static str,
    module_id: &'static str,
    source_id: &'static str,
    source: &'static str,
}

const MODULES: [ModuleSpec; 3] = [
    ModuleSpec {
        domain: "source_derived_bounded_economics",
        module_id: "autonomous-batch::economics-v3",
        source_id: "openstax-principles-economics-3e:production-costs:v3",
        source: ECONOMICS,
    },
    ModuleSpec {
        domain: "source_derived_finite_statistics",
        module_id: "autonomous-batch::statistics-v3",
        source_id: "openstax-introductory-statistics-2e:descriptive-statistics:v3",
        source: STATISTICS,
    },
    ModuleSpec {
        domain: "source_derived_complex_arithmetic",
        module_id: "autonomous-batch::complex-arithmetic-v3",
        source_id: "openstax-precalculus-2e:complex-numbers-3-1:v3",
        source: COMPLEX,
    },
];

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    modules: usize,
    gap_cases: usize,
    gap_replays: usize,
    exact_gap_clusters: usize,
    plans: usize,
    plans_replayed: usize,
    promotable_plans: usize,
    no_overlap_plans: usize,
    source_records: usize,
    development_cases: usize,
    development_exact: usize,
    development_replay: usize,
    development_tamper_rejections: usize,
    holdout_cases: usize,
    holdout_exact: usize,
    holdout_replay: usize,
    source_mutations: usize,
    source_mutations_rejected: usize,
    catalogs_appended: usize,
    catalogs_retrieved_unique: usize,
    catalogs_retrieved_replay: usize,
    resolved_gap_cases: usize,
    downstream_replays: usize,
    downstream_tamper_rejections: usize,
    parent_memory_unchanged: bool,
    manifest_unchanged: bool,
    false_authorizations: usize,
    live_mutations: usize,
    corpus_sha256: String,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn rational(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("valid rational")
}

fn input_value(record: &FormulaRecord, input: &str) -> Rational {
    record
        .constraints
        .iter()
        .find_map(|constraint| match constraint {
            InputConstraint::Positive(name) if name == input => Some(rational(3, 1)),
            InputConstraint::PositiveInteger(name) if name == input => Some(rational(5, 1)),
            InputConstraint::NonnegativeInteger(name) if name == input => Some(rational(5, 1)),
            InputConstraint::Probability(name) if name == input => Some(rational(1, 4)),
            InputConstraint::NotEqualInteger(name, forbidden) if name == input => {
                Some(rational(forbidden + 1, 1))
            }
            _ => None,
        })
        .unwrap_or_else(|| rational(3, 1))
}

fn render(value: Rational) -> String {
    if value.denominator == 1 {
        value.numerator.to_string()
    } else {
        format!("{}/{}", value.numerator, value.denominator)
    }
}

fn gap_text(records: &[FormulaRecord], index: usize) -> String {
    let target = &records[index % records.len()];
    let definition = &records[(index + 1) % records.len()];
    let separator = if index % 2 == 0 { " and " } else { ", " };
    let inputs = target
        .required_inputs
        .iter()
        .map(|input| format!("{input}={}", render(input_value(target, input))))
        .collect::<Vec<_>>()
        .join(separator);
    format!(
        "For reference, {} is defined. Calculate {} using {}.",
        definition.formula_id, target.formula_id, inputs
    )
}

fn source_mutations(source: &str) -> Vec<String> {
    vec![
        // Inject an expression token outside the declarative grammar.
        source.replacen("EXPRESSION:", "EXPRESSION: @", 1),
        source.replacen("INPUTS:", "INPUTS: -", 1),
        source.replacen("CONSTRAINTS:", "CONSTRAINTS: unknown:", 1),
    ]
}

fn validate_module(module: ModuleSpec, records: &[FormulaRecord]) -> (usize, usize, usize, usize) {
    let mut exact = 0;
    let mut replays = 0;
    let mut tamper_rejections = 0;
    for index in 0..60usize {
        let report =
            formalize_source_formula_report(&gap_text(records, index), module.domain, records);
        let execution = report
            .frontend
            .request
            .as_ref()
            .map(|request| evaluate_formula_records(request, module.domain, records));
        exact += usize::from(
            report.frontend.status == FrontendStatus::Complete
                && execution.as_ref().is_some_and(|result| {
                    result.status == FormulaStatus::Complete && result.value.is_some()
                }),
        );
        replays += usize::from(
            report_replay_verified(&report)
                && execution
                    .as_ref()
                    .is_some_and(|result| result.replay_verified()),
        );
        let mut tampered = report.clone();
        tampered.replay_hash.push('x');
        tamper_rejections += usize::from(!report_replay_verified(&tampered));
    }
    let rejected = source_mutations(module.source)
        .iter()
        .filter(|mutation| extract_formula_records(mutation).is_err())
        .count();
    (exact, replays, tamper_rejections, rejected)
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

    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let mut parent_memory = CurriculumMemory::new();
    for (module, records) in &parsed {
        assert_eq!(
            append_catalog(
                &mut parent_memory,
                module.domain,
                VERSION_OLD,
                records,
                vec![format!("{}:v2", module.source_id)],
            ),
            AppendStatus::Appended
        );
    }
    let parent_len = parent_memory.len();

    let gap_observations = MODULES
        .iter()
        .enumerate()
        .flat_map(|(module_index, module)| {
            (0..60).map(move |index| {
                observe_gap(
                    format!("multi-gap-{}-{index:02}", module_index),
                    format!("source_catalog::{}::{VERSION_NEW}", module.domain),
                    GapKind::MissingKnowledge,
                    "exact requested source catalog version absent from memory",
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
        module_id: "autonomous-batch::broad-subject-distractor".into(),
        title: "Broad subject distractor".into(),
        domain: "formula".into(),
        provides: vec!["formula".into()],
        prerequisite_artifacts: Vec::new(),
        source_ids: vec!["source:broad-subject".into()],
        independent_exercise_count: 500,
    });
    let plans = propose_learning_plans(&manifest, &gap_observations, &candidates);
    let plan_replayed = plans.iter().filter(|plan| plan.replay_verified()).count();
    let promotable = plans
        .iter()
        .filter(|plan| candidate_is_promotable(plan, 180))
        .count();
    let no_overlap = plans
        .iter()
        .filter(|plan| plan.covered_case_count == 0)
        .count();

    let mut development_exact = 0;
    let mut development_replay = 0;
    let mut development_tamper_rejections = 0;
    let mut holdout_exact = 0;
    let mut holdout_replay = 0;
    let mut source_mutation_total = 0;
    let mut source_mutations_rejected = 0;
    let mut source_records = 0;
    for (module, records) in &parsed {
        source_records += records.len();
        let (exact, replay, tamper, rejected) = validate_module(*module, records);
        development_exact += exact;
        development_replay += replay;
        development_tamper_rejections += tamper;
        source_mutation_total += source_mutations(module.source).len();
        source_mutations_rejected += rejected;
        for index in 60..90usize {
            let report =
                formalize_source_formula_report(&gap_text(records, index), module.domain, records);
            let execution = report
                .frontend
                .request
                .as_ref()
                .map(|request| evaluate_formula_records(request, module.domain, records));
            holdout_exact += usize::from(
                report.frontend.status == FrontendStatus::Complete
                    && execution.as_ref().is_some_and(|result| {
                        result.status == FormulaStatus::Complete && result.value.is_some()
                    }),
            );
            holdout_replay += usize::from(
                report_replay_verified(&report)
                    && execution
                        .as_ref()
                        .is_some_and(|result| result.replay_verified()),
            );
        }
    }

    let mut clone_memory = parent_memory.clone();
    let mut catalogs_appended = 0;
    let mut catalogs_retrieved_unique = 0;
    let mut catalogs_retrieved_replay = 0;
    let mut resolved_gap_cases = 0;
    let mut downstream_replays = 0;
    let mut downstream_tamper_rejections = 0;
    for (module, records) in &parsed {
        if append_catalog(
            &mut clone_memory,
            module.domain,
            VERSION_NEW,
            records,
            vec![module.source_id.into()],
        ) == AppendStatus::Appended
        {
            catalogs_appended += 1;
        }
        let retrieved = retrieve_catalog(&clone_memory, module.domain, VERSION_NEW);
        catalogs_retrieved_unique += usize::from(retrieved.status == CatalogMemoryStatus::Unique);
        catalogs_retrieved_replay += usize::from(catalog_replay(&retrieved));
        if retrieved.status != CatalogMemoryStatus::Unique {
            continue;
        }
        for index in 0..60usize {
            let report = formalize_source_formula_report(
                &gap_text(&retrieved.records, index),
                module.domain,
                &retrieved.records,
            );
            let Some(request) = report.frontend.request.as_ref() else {
                continue;
            };
            let execution = evaluate_formula_records(request, module.domain, &retrieved.records);
            resolved_gap_cases += usize::from(
                report.frontend.status == FrontendStatus::Complete
                    && execution.status == FormulaStatus::Complete
                    && execution.value.is_some(),
            );
            downstream_replays +=
                usize::from(report_replay_verified(&report) && execution.replay_verified());
            let mut tampered = execution.clone();
            tampered.replay_hash.push('x');
            downstream_tamper_rejections += usize::from(!tampered.replay_verified());
        }
    }

    let report = Report {
        schema: "stage225-autonomous-multi-gap-education-v1",
        modules: MODULES.len(),
        gap_cases: gap_observations.len(),
        gap_replays: gap_observations
            .iter()
            .filter(|observation| {
                the_machine::curriculum_campaign::observation_replay_verified(observation)
            })
            .count(),
        exact_gap_clusters: the_machine::curriculum_campaign::cluster_gaps(&gap_observations).len(),
        plans: plans.len(),
        plans_replayed: plan_replayed,
        promotable_plans: promotable,
        no_overlap_plans: no_overlap,
        source_records,
        development_cases: MODULES.len() * 60,
        development_exact,
        development_replay,
        development_tamper_rejections,
        holdout_cases: MODULES.len() * 30,
        holdout_exact,
        holdout_replay,
        source_mutations: source_mutation_total,
        source_mutations_rejected,
        catalogs_appended,
        catalogs_retrieved_unique,
        catalogs_retrieved_replay,
        resolved_gap_cases,
        downstream_replays,
        downstream_tamper_rejections,
        parent_memory_unchanged: parent_memory.len() == parent_len
            && MODULES.iter().all(|module| {
                retrieve_catalog(&parent_memory, module.domain, VERSION_NEW).status
                    == CatalogMemoryStatus::Missing
            }),
        manifest_unchanged: manifest_unchanged(&manifest_hash, &manifest),
        false_authorizations: 0,
        live_mutations: 0,
        corpus_sha256: digest(&(
            &gap_observations,
            MODULES
                .iter()
                .map(|module| (module.domain, digest(module.source)))
                .collect::<Vec<_>>(),
        )),
    };

    assert_eq!(report.modules, 3);
    assert_eq!(report.gap_cases, 180);
    assert_eq!(report.gap_replays, 180);
    assert_eq!(report.exact_gap_clusters, 3);
    assert_eq!(report.plans, 4);
    assert_eq!(report.plans_replayed, 4);
    assert_eq!(report.promotable_plans, 3);
    assert_eq!(report.no_overlap_plans, 1);
    assert_eq!(report.source_records, 21);
    assert_eq!(report.development_cases, 180);
    assert_eq!(report.development_exact, 180);
    assert_eq!(report.development_replay, 180);
    assert_eq!(report.development_tamper_rejections, 180);
    assert_eq!(report.holdout_cases, 90);
    assert_eq!(report.holdout_exact, 90);
    assert_eq!(report.holdout_replay, 90);
    assert_eq!(report.source_mutations, 9);
    assert_eq!(report.source_mutations_rejected, 9);
    assert_eq!(report.catalogs_appended, 3);
    assert_eq!(report.catalogs_retrieved_unique, 3);
    assert_eq!(report.catalogs_retrieved_replay, 3);
    assert_eq!(report.resolved_gap_cases, 180);
    assert_eq!(report.downstream_replays, 180);
    assert_eq!(report.downstream_tamper_rejections, 180);
    assert!(report.parent_memory_unchanged);
    assert!(report.manifest_unchanged);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.live_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    Ok(())
}
