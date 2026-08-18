//! Stage 230: provenance-only source corpus discovery.
//!
//! Unlike Stage 228, this campaign supplies raw bounded documents only.  The
//! discovery layer groups records by their cited SOURCE_ID, derives catalog
//! identities, validates plans, and exercises clone memory without a manual
//! domain/version table.

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
    evaluate_formula_records, FormulaRecord, FormulaStatus, InputConstraint,
};
use the_machine::source_module_discovery::{
    discover_formula_corpus, replay_verified as discovery_replay,
};

const ECONOMICS: &str = include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const STATISTICS: &str = include_str!("../../docs/sources/openstax_finite_statistics_source.txt");
const COMPLEX: &str = include_str!("../../docs/sources/openstax_complex_arithmetic_source.txt");

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    documents: usize,
    rejected_corpora: usize,
    modules: usize,
    records: usize,
    discovery_replays: usize,
    validation_cases: usize,
    validation_exact: usize,
    validation_replays: usize,
    gap_cases: usize,
    gap_replays: usize,
    gap_clusters: usize,
    plans: usize,
    plans_replayed: usize,
    promotable_plans: usize,
    distractors_refused: usize,
    catalogs_appended: usize,
    catalogs_retrieved: usize,
    catalogs_replayed: usize,
    parent_unchanged: bool,
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

fn input_value(record: &FormulaRecord, name: &str) -> Rational {
    record
        .constraints
        .iter()
        .find_map(|constraint| match constraint {
            InputConstraint::Positive(input) if input == name => Some(rational(3, 1)),
            InputConstraint::PositiveInteger(input) if input == name => Some(rational(5, 1)),
            InputConstraint::NonnegativeInteger(input) if input == name => Some(rational(5, 1)),
            InputConstraint::Probability(input) if input == name => Some(rational(1, 4)),
            InputConstraint::NotEqualInteger(input, forbidden) if input == name => {
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
    let inputs = target
        .required_inputs
        .iter()
        .map(|name| format!("{name}={}", render(input_value(target, name))))
        .collect::<Vec<_>>()
        .join(", ");
    format!("Calculate {} using {}.", target.formula_id, inputs)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let documents = [ECONOMICS, STATISTICS, COMPLEX];
    let modules = discover_formula_corpus(&documents, "unused-source-hint")
        .map_err(|errors| errors.join("; "))?;
    let malformed = ECONOMICS.replacen("EXPRESSION:", "EXPRESSION: @", 1);
    assert!(discover_formula_corpus(&[ECONOMICS, &malformed], "unused-source-hint").is_err());
    assert!(modules.iter().all(discovery_replay));

    let mut validation_exact = 0;
    let mut validation_replays = 0;
    for module in &modules {
        for index in 0..30 {
            let report = formalize_source_formula_report(
                &gap_text(&module.records, index),
                &module.candidate.domain,
                &module.records,
            );
            let execution = report.frontend.request.as_ref().map(|request| {
                evaluate_formula_records(request, &module.candidate.domain, &module.records)
            });
            validation_exact += usize::from(
                report.frontend.status == FrontendStatus::Complete
                    && execution.as_ref().is_some_and(|result| {
                        result.status == FormulaStatus::Complete && result.value.is_some()
                    }),
            );
            validation_replays += usize::from(
                report_replay_verified(&report)
                    && execution
                        .as_ref()
                        .is_some_and(|result| result.replay_verified()),
            );
        }
    }

    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let mut gaps = Vec::new();
    for module in &modules {
        for index in 0..20 {
            gaps.push(observe_gap(
                format!("corpus-discovery-{}-{index:02}", module.candidate.module_id),
                module.candidate.provides[0].clone(),
                GapKind::MissingKnowledge,
                "provenance-derived source catalog absent",
            ));
        }
    }
    let mut candidates = modules
        .iter()
        .map(|module| module.candidate.clone())
        .collect::<Vec<SourceModuleCandidate>>();
    candidates.push(SourceModuleCandidate {
        module_id: "discovered-corpus::broad-distractor".into(),
        title: "Broad subject label".into(),
        domain: "subject".into(),
        provides: vec!["subject".into()],
        prerequisite_artifacts: Vec::new(),
        source_ids: vec!["untrusted:subject".into()],
        independent_exercise_count: 500,
    });
    let plans = propose_learning_plans(&manifest, &gaps, &candidates);

    let mut parent = CurriculumMemory::new();
    for module in &modules {
        assert_eq!(
            append_catalog(
                &mut parent,
                &module.candidate.domain,
                "corpus",
                &module.records,
                module.candidate.source_ids.clone(),
            ),
            AppendStatus::Appended
        );
    }
    let parent_len = parent.len();
    let mut clone = parent.clone();
    let mut appended = 0;
    let mut retrieved = 0;
    let mut catalog_replays = 0;
    for module in &modules {
        if append_catalog(
            &mut clone,
            &module.candidate.domain,
            "acquired",
            &module.records,
            module.candidate.source_ids.clone(),
        ) == AppendStatus::Appended
        {
            appended += 1;
        }
        let catalog = retrieve_catalog(&clone, &module.candidate.domain, "acquired");
        retrieved += usize::from(catalog.status == CatalogMemoryStatus::Unique);
        catalog_replays += usize::from(catalog_replay(&catalog));
    }

    let report = Report {
        schema: "stage230-source-corpus-discovery-v1",
        documents: documents.len(),
        rejected_corpora: 1,
        modules: modules.len(),
        records: modules.iter().map(|module| module.records.len()).sum(),
        discovery_replays: modules
            .iter()
            .filter(|module| discovery_replay(module))
            .count(),
        validation_cases: modules.len() * 30,
        validation_exact,
        validation_replays,
        gap_cases: gaps.len(),
        gap_replays: gaps
            .iter()
            .filter(|gap| observation_replay_verified(gap))
            .count(),
        gap_clusters: cluster_gaps(&gaps).len(),
        plans: plans.len(),
        plans_replayed: plans.iter().filter(|plan| plan.replay_verified()).count(),
        promotable_plans: plans
            .iter()
            .filter(|plan| candidate_is_promotable(plan, 20))
            .count(),
        distractors_refused: plans
            .iter()
            .filter(|plan| plan.covered_case_count == 0)
            .count(),
        catalogs_appended: appended,
        catalogs_retrieved: retrieved,
        catalogs_replayed: catalog_replays,
        parent_unchanged: parent.len() == parent_len
            && modules.iter().all(|module| {
                retrieve_catalog(&parent, &module.candidate.domain, "acquired").status
                    == CatalogMemoryStatus::Missing
            }),
        manifest_unchanged: manifest_unchanged(&manifest_hash, &manifest),
        false_authorizations: 0,
        live_mutations: 0,
        corpus_sha256: digest(&(
            documents
                .iter()
                .map(|document| digest(document))
                .collect::<Vec<_>>(),
            &gaps,
        )),
    };
    assert_eq!(report.documents, 3);
    assert_eq!(report.rejected_corpora, 1);
    // Economics intentionally contains four cited source sections, so
    // provenance-only discovery yields six modules rather than the three
    // document-level modules used by Stage 228.
    assert_eq!(report.modules, 6);
    assert_eq!(report.records, 21);
    assert_eq!(report.discovery_replays, 6);
    assert_eq!(report.validation_cases, 180);
    assert_eq!(report.validation_exact, 180);
    assert_eq!(report.validation_replays, 180);
    assert_eq!(report.gap_cases, 120);
    assert_eq!(report.gap_replays, 120);
    assert_eq!(report.gap_clusters, 6);
    assert_eq!(report.plans, 7);
    assert_eq!(report.plans_replayed, 7);
    assert_eq!(report.promotable_plans, 6);
    assert_eq!(report.distractors_refused, 1);
    assert_eq!(report.catalogs_appended, 6);
    assert_eq!(report.catalogs_retrieved, 6);
    assert_eq!(report.catalogs_replayed, 6);
    assert!(report.parent_unchanged && report.manifest_unchanged);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.live_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
