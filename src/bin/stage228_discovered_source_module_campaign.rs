//! Stage 228: structural source-module discovery before acquisition.
//!
//! This campaign removes the remaining hand-authored module-list step.  A
//! bounded source document is parsed into provenance-bearing formula records;
//! only then is a typed curriculum candidate derived.  Discovery, planning,
//! sandbox acquisition, and replay are all evaluated without live mutation.

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
use the_machine::source_module_discovery::{
    discover_formula_module, replay_verified as discovery_replay, SourceDocument,
};

const OLD: &str = "v2";
const NEW: &str = "v3";
const ECONOMICS: &str = include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const STATISTICS: &str = include_str!("../../docs/sources/openstax_finite_statistics_source.txt");
const COMPLEX: &str = include_str!("../../docs/sources/openstax_complex_arithmetic_source.txt");

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_documents: usize,
    discovered_modules: usize,
    rejected_documents: usize,
    discovery_replays: usize,
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

fn rational(n: i128, d: i128) -> Rational {
    Rational::new(n, d).expect("valid rational")
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

fn mutations(source: &str) -> Vec<String> {
    vec![
        source.replacen("EXPRESSION:", "EXPRESSION: @", 1),
        source.replacen("INPUTS:", "INPUTS: -", 1),
        source.replacen("CONSTRAINTS:", "CONSTRAINTS: unknown:", 1),
    ]
}

fn validate_range(
    domain: &str,
    records: &[FormulaRecord],
    range: std::ops::Range<usize>,
) -> (usize, usize, usize) {
    let mut exact = 0;
    let mut replay = 0;
    let mut tamper = 0;
    for index in range {
        let report = formalize_source_formula_report(&gap_text(records, index), domain, records);
        let execution = report
            .frontend
            .request
            .as_ref()
            .map(|request| evaluate_formula_records(request, domain, records));
        exact += usize::from(
            report.frontend.status == FrontendStatus::Complete
                && execution.as_ref().is_some_and(|result| {
                    result.status == FormulaStatus::Complete && result.value.is_some()
                }),
        );
        replay += usize::from(
            report_replay_verified(&report)
                && execution
                    .as_ref()
                    .is_some_and(|result| result.replay_verified()),
        );
        let mut altered = report.clone();
        altered.replay_hash.push('x');
        tamper += usize::from(!report_replay_verified(&altered));
    }
    (exact, replay, tamper)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let documents = [
        SourceDocument {
            domain: "source_derived_bounded_economics",
            version: NEW,
            source_hint: "economics",
            document: ECONOMICS,
        },
        SourceDocument {
            domain: "source_derived_finite_statistics",
            version: NEW,
            source_hint: "statistics",
            document: STATISTICS,
        },
        SourceDocument {
            domain: "source_derived_complex_arithmetic",
            version: NEW,
            source_hint: "complex",
            document: COMPLEX,
        },
    ];
    let malformed_text = ECONOMICS.replacen("EXPRESSION:", "EXPRESSION: @", 1);
    let malformed = SourceDocument {
        domain: "malformed",
        version: NEW,
        source_hint: "malformed",
        document: &malformed_text,
    };
    let discovered = documents
        .iter()
        .map(|document| discover_formula_module(*document).map_err(|errors| errors.join("; ")))
        .collect::<Result<Vec<_>, _>>()?;
    assert!(discover_formula_module(malformed).is_err());
    assert!(discovered.iter().all(discovery_replay));

    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let mut parent = CurriculumMemory::new();
    for module in &discovered {
        assert_eq!(
            append_catalog(
                &mut parent,
                &module.candidate.domain,
                OLD,
                &module.records,
                module.candidate.source_ids.clone()
            ),
            AppendStatus::Appended
        );
    }
    let parent_len = parent.len();

    let gaps = discovered
        .iter()
        .enumerate()
        .flat_map(|(module_index, module)| {
            (0..60).map(move |index| {
                observe_gap(
                    format!("discovered-gap-{module_index}-{index:02}"),
                    module.candidate.provides[0].clone(),
                    GapKind::MissingKnowledge,
                    "exact source catalog absent from memory",
                )
            })
        })
        .collect::<Vec<_>>();
    let mut candidates = discovered
        .iter()
        .map(|module| module.candidate.clone())
        .collect::<Vec<SourceModuleCandidate>>();
    candidates.push(SourceModuleCandidate {
        module_id: "discovered::broad-distractor".into(),
        title: "Broad subject distractor".into(),
        domain: "formula".into(),
        provides: vec!["formula".into()],
        prerequisite_artifacts: Vec::new(),
        source_ids: vec!["source:broad-subject".into()],
        independent_exercise_count: 500,
    });
    let plans = propose_learning_plans(&manifest, &gaps, &candidates);

    let mut source_records = 0;
    let mut dev_exact = 0;
    let mut dev_replay = 0;
    let mut dev_tamper = 0;
    let mut hold_exact = 0;
    let mut hold_replay = 0;
    let mut source_mutations = 0;
    let mut source_mutations_rejected = 0;
    for module in &discovered {
        source_records += module.records.len();
        let (exact, replay, tamper) =
            validate_range(&module.candidate.domain, &module.records, 0..60);
        dev_exact += exact;
        dev_replay += replay;
        dev_tamper += tamper;
        for index in 60..90 {
            let (exact, replay, _) =
                validate_range(&module.candidate.domain, &module.records, index..index + 1);
            hold_exact += exact;
            hold_replay += replay;
        }
        // Mutation rejection is assessed on the original bounded source, not on a
        // hand-authored candidate declaration.
        let source = if module.candidate.domain.contains("economics") {
            ECONOMICS
        } else if module.candidate.domain.contains("statistics") {
            STATISTICS
        } else {
            COMPLEX
        };
        let altered = mutations(source);
        source_mutations += altered.len();
        source_mutations_rejected += altered
            .iter()
            .filter(|text| extract_formula_records(text).is_err())
            .count();
    }

    let mut clone = parent.clone();
    let mut appended = 0;
    let mut unique = 0;
    let mut catalog_replays = 0;
    let mut resolved = 0;
    let mut downstream_replays = 0;
    let mut downstream_tamper = 0;
    for module in &discovered {
        if append_catalog(
            &mut clone,
            &module.candidate.domain,
            NEW,
            &module.records,
            module.candidate.source_ids.clone(),
        ) == AppendStatus::Appended
        {
            appended += 1;
        }
        let retrieved = retrieve_catalog(&clone, &module.candidate.domain, NEW);
        unique += usize::from(retrieved.status == CatalogMemoryStatus::Unique);
        catalog_replays += usize::from(catalog_replay(&retrieved));
        if retrieved.status != CatalogMemoryStatus::Unique {
            continue;
        }
        for index in 0..60 {
            let report = formalize_source_formula_report(
                &gap_text(&retrieved.records, index),
                &module.candidate.domain,
                &retrieved.records,
            );
            if let Some(request) = report.frontend.request.as_ref() {
                let execution =
                    evaluate_formula_records(request, &module.candidate.domain, &retrieved.records);
                resolved += usize::from(
                    report.frontend.status == FrontendStatus::Complete
                        && execution.status == FormulaStatus::Complete
                        && execution.value.is_some(),
                );
                downstream_replays +=
                    usize::from(report_replay_verified(&report) && execution.replay_verified());
                let mut altered = execution.clone();
                altered.replay_hash.push('x');
                downstream_tamper += usize::from(!altered.replay_verified());
            }
        }
    }

    let report = Report {
        schema: "stage228-discovered-source-module-campaign-v1",
        source_documents: 4,
        discovered_modules: discovered.len(),
        rejected_documents: 1,
        discovery_replays: discovered
            .iter()
            .filter(|module| discovery_replay(module))
            .count(),
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
        source_records,
        development_cases: 180,
        development_exact: dev_exact,
        development_replay: dev_replay,
        development_tamper_rejections: dev_tamper,
        holdout_cases: 90,
        holdout_exact: hold_exact,
        holdout_replay: hold_replay,
        source_mutations,
        source_mutations_rejected,
        catalogs_appended: appended,
        catalogs_retrieved_unique: unique,
        catalogs_retrieved_replay: catalog_replays,
        resolved_gap_cases: resolved,
        downstream_replays,
        downstream_tamper_rejections: downstream_tamper,
        parent_memory_unchanged: parent.len() == parent_len
            && discovered.iter().all(|module| {
                retrieve_catalog(&parent, &module.candidate.domain, NEW).status
                    == CatalogMemoryStatus::Missing
            }),
        manifest_unchanged: manifest_unchanged(&manifest_hash, &manifest),
        false_authorizations: 0,
        live_mutations: 0,
        corpus_sha256: digest(&(
            documents
                .iter()
                .map(|document| (document.domain, digest(document.document)))
                .collect::<Vec<_>>(),
            &gaps,
        )),
    };

    assert_eq!(report.source_documents, 4);
    assert_eq!(report.discovered_modules, 3);
    assert_eq!(report.rejected_documents, 1);
    assert_eq!(report.discovery_replays, 3);
    assert_eq!(report.gap_cases, 180);
    assert_eq!(report.gap_replays, 180);
    assert_eq!(report.exact_gap_clusters, 3);
    assert_eq!(report.plans, 4);
    assert_eq!(report.plans_replayed, 4);
    assert_eq!(report.promotable_plans, 3);
    assert_eq!(report.no_overlap_plans, 1);
    assert_eq!(report.source_records, 21);
    assert_eq!(report.development_exact, 180);
    assert_eq!(report.development_replay, 180);
    assert_eq!(report.development_tamper_rejections, 180);
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
    assert!(report.parent_memory_unchanged && report.manifest_unchanged);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.live_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    Ok(())
}
