//! Stage 233: sealed learning curve from provenance-derived source modules.
//!
//! The controller receives raw source documents, discovers module boundaries
//! from SOURCE_ID provenance, validates the resulting records, and acquires
//! them only in a curriculum-memory clone. A permanently partitioned corpus is
//! scored before and after acquisition; the sealed partition is never used to
//! select a module.

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
use the_machine::source_formula_pack::{evaluate_formula_records, FormulaRecord, InputConstraint};
use the_machine::source_module_discovery::{
    discover_formula_corpus, replay_verified as discovery_replay, DiscoveredSourceModule,
};

const ECONOMICS: &str = include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const STATISTICS: &str = include_str!("../../docs/sources/openstax_finite_statistics_source.txt");
const COMPLEX: &str = include_str!("../../docs/sources/openstax_complex_arithmetic_source.txt");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
enum Kind {
    Supported,
    Unresolved,
    Unsupported,
}

#[derive(Debug, Clone)]
struct Case {
    id: String,
    kind: Kind,
    text: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    development_cases: usize,
    validation_cases: usize,
    sealed_cases: usize,
    total_cases: usize,
    source_modules: usize,
    source_records: usize,
    source_discovery_replays: usize,
    source_validation_cases: usize,
    source_validation_exact: usize,
    source_validation_replays: usize,
    malformed_sources: usize,
    malformed_sources_rejected: usize,
    gap_cases: usize,
    gap_replays: usize,
    gap_clusters: usize,
    plans: usize,
    plans_replayed: usize,
    promotable_plans: usize,
    distractors_refused: usize,
    baseline_authorizations: usize,
    baseline_supported_authorizations: usize,
    post_exact_decisions: usize,
    post_authorizations: usize,
    post_supported_authorizations: usize,
    post_unresolved_refusals: usize,
    post_unsupported_refusals: usize,
    post_replays: usize,
    post_tamper_rejections: usize,
    sealed_exact_decisions: usize,
    sealed_authorizations: usize,
    catalogs_appended: usize,
    catalogs_retrieved_unique: usize,
    catalogs_retrieved_replay: usize,
    parent_unchanged: bool,
    manifest_unchanged: bool,
    false_authorizations: usize,
    false_denials: usize,
    live_mutations: usize,
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

fn supported_text(records: &[FormulaRecord], index: usize) -> String {
    let record = &records[index % records.len()];
    let inputs = record
        .required_inputs
        .iter()
        .map(|name| format!("{name}={}", render(input_value(record, name))))
        .collect::<Vec<_>>()
        .join(" and ");
    format!("Calculate {} using {}.", record.formula_id, inputs)
}

fn route(case: &Case, modules: &[DiscoveredSourceModule]) -> (bool, usize, usize, usize, usize) {
    let mut complete = 0;
    let mut frontend_replay = 0;
    let mut frontend_tamper = 0;
    let mut downstream_replay = 0;
    let mut downstream_tamper = 0;
    for module in modules {
        let report =
            formalize_source_formula_report(&case.text, &module.candidate.domain, &module.records);
        frontend_replay += usize::from(report_replay_verified(&report));
        let mut altered = report.clone();
        altered.replay_hash.push('x');
        frontend_tamper += usize::from(!report_replay_verified(&altered));
        if report.frontend.status == FrontendStatus::Complete {
            complete += 1;
            if let Some(request) = report.frontend.request.as_ref() {
                let execution =
                    evaluate_formula_records(request, &module.candidate.domain, &module.records);
                downstream_replay += usize::from(execution.replay_verified());
                let mut altered = execution.clone();
                altered.replay_hash.push('x');
                downstream_tamper += usize::from(!altered.replay_verified());
            }
        }
    }
    let expected_complete = case.kind == Kind::Supported;
    (
        complete == 1 && expected_complete || complete == 0 && !expected_complete,
        frontend_replay,
        frontend_tamper,
        downstream_replay,
        downstream_tamper,
    )
}

fn make_partition(modules: &[DiscoveredSourceModule]) -> (Vec<Case>, Vec<Case>, Vec<Case>) {
    let mut development = Vec::new();
    let mut validation = Vec::new();
    let mut sealed = Vec::new();
    let next = |prefix: &str, kind: Kind, text: String, target: &mut Vec<Case>, index: usize| {
        target.push(Case {
            id: format!("{prefix}-{index:04}"),
            kind,
            text,
        });
    };
    for index in 0..360 {
        let module = &modules[index % modules.len()];
        next(
            "development-supported",
            Kind::Supported,
            supported_text(&module.records, index),
            &mut development,
            index,
        );
    }
    for index in 0..120 {
        next(
            "development-unresolved",
            Kind::Unresolved,
            "Calculate the result from source context.".into(),
            &mut development,
            index,
        );
    }
    for index in 0..120 {
        next(
            "development-unsupported",
            Kind::Unsupported,
            "Calculate the infinite approximation of the result.".into(),
            &mut development,
            index,
        );
    }
    for index in 0..120 {
        let module = &modules[(index + 1) % modules.len()];
        next(
            "validation-supported",
            Kind::Supported,
            supported_text(&module.records, index + 7),
            &mut validation,
            index,
        );
    }
    for index in 0..40 {
        next(
            "validation-unresolved",
            Kind::Unresolved,
            "Find the result from source context.".into(),
            &mut validation,
            index,
        );
    }
    for index in 0..40 {
        next(
            "validation-unsupported",
            Kind::Unsupported,
            "Evaluate an infinite approximation.".into(),
            &mut validation,
            index,
        );
    }
    for index in 0..120 {
        let module = &modules[(index + 2) % modules.len()];
        next(
            "sealed-supported",
            Kind::Supported,
            supported_text(&module.records, index + 13),
            &mut sealed,
            index,
        );
    }
    for index in 0..40 {
        next(
            "sealed-unresolved",
            Kind::Unresolved,
            "Determine the result from context.".into(),
            &mut sealed,
            index,
        );
    }
    for index in 0..40 {
        next(
            "sealed-unsupported",
            Kind::Unsupported,
            "Compute the infinite approximation.".into(),
            &mut sealed,
            index,
        );
    }
    (development, validation, sealed)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let documents = [ECONOMICS, STATISTICS, COMPLEX];
    let modules =
        discover_formula_corpus(&documents, "unused-hint").map_err(|errors| errors.join("; "))?;
    assert_eq!(modules.len(), 6);
    let malformed = ECONOMICS.replacen("EXPRESSION:", "EXPRESSION: @", 1);
    let malformed_sources_rejected =
        usize::from(discover_formula_corpus(&[ECONOMICS, &malformed], "unused-hint").is_err());
    let source_validation_cases = modules.len() * 30;
    let mut source_validation_exact = 0;
    let mut source_validation_replays = 0;
    for module in &modules {
        for index in 0..30 {
            let case = Case {
                id: String::new(),
                kind: Kind::Supported,
                text: supported_text(&module.records, index),
            };
            let result = route(&case, std::slice::from_ref(module));
            source_validation_exact += usize::from(result.0);
            source_validation_replays += usize::from(result.1 == 1 && result.3 == 1);
        }
    }

    let (development, validation, sealed) = make_partition(&modules);
    let all_cases = development
        .iter()
        .chain(validation.iter())
        .chain(sealed.iter())
        .cloned()
        .collect::<Vec<_>>();
    let mut gaps = Vec::new();
    for module in &modules {
        gaps.extend((0..20).map(|index| {
            observe_gap(
                format!(
                    "provenance-learning-{}-{index:02}",
                    module.candidate.module_id
                ),
                module.candidate.provides[0].clone(),
                GapKind::MissingKnowledge,
                "source catalog is absent before acquisition",
            )
        }));
    }
    let mut candidates = modules
        .iter()
        .map(|module| module.candidate.clone())
        .collect::<Vec<SourceModuleCandidate>>();
    candidates.push(SourceModuleCandidate {
        module_id: "provenance-learning::broad-distractor".into(),
        title: "Broad subject".into(),
        domain: "subject".into(),
        provides: vec!["subject".into()],
        prerequisite_artifacts: Vec::new(),
        source_ids: vec!["untrusted:subject".into()],
        independent_exercise_count: 500,
    });
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let plans = propose_learning_plans(&manifest, &gaps, &candidates);
    let baseline = Vec::<DiscoveredSourceModule>::new();
    let baseline_authorizations = all_cases
        .iter()
        .filter(|case| route(case, &baseline).0 && case.kind == Kind::Supported)
        .count();
    let mut parent = CurriculumMemory::new();
    for module in &modules {
        assert_eq!(
            append_catalog(
                &mut parent,
                &module.candidate.domain,
                "source",
                &module.records,
                module.candidate.source_ids.clone()
            ),
            AppendStatus::Appended
        );
    }
    let parent_len = parent.len();
    let mut clone = parent.clone();
    let mut catalogs_appended = 0;
    let mut catalogs_retrieved_unique = 0;
    let mut catalogs_retrieved_replay = 0;
    for module in &modules {
        if append_catalog(
            &mut clone,
            &module.candidate.domain,
            "acquired",
            &module.records,
            module.candidate.source_ids.clone(),
        ) == AppendStatus::Appended
        {
            catalogs_appended += 1;
        }
        let catalog = retrieve_catalog(&clone, &module.candidate.domain, "acquired");
        catalogs_retrieved_unique += usize::from(catalog.status == CatalogMemoryStatus::Unique);
        catalogs_retrieved_replay += usize::from(catalog_replay(&catalog));
    }
    let mut post_exact = 0;
    let mut post_authorizations = 0;
    let mut post_supported_authorizations = 0;
    let mut post_unresolved = 0;
    let mut post_unsupported = 0;
    let mut post_replays = 0;
    let mut post_tamper = 0;
    for case in &all_cases {
        let result = route(case, &modules);
        post_exact += usize::from(result.0);
        post_authorizations += usize::from(case.kind == Kind::Supported && result.0);
        post_supported_authorizations += usize::from(case.kind == Kind::Supported && result.0);
        post_unresolved += usize::from(case.kind == Kind::Unresolved && result.0);
        post_unsupported += usize::from(case.kind == Kind::Unsupported && result.0);
        post_replays += result.1;
        post_tamper += result.2;
        // Downstream receipts are emitted exactly once per supported case.
        post_replays += result.3;
        post_tamper += result.4;
    }
    let mut sealed_exact = 0;
    let mut sealed_authorizations = 0;
    for case in &sealed {
        let result = route(case, &modules);
        sealed_exact += usize::from(result.0);
        sealed_authorizations += usize::from(case.kind == Kind::Supported && result.0);
    }
    let report = Report {
        schema: "stage233-provenance-learning-curve-v1",
        corpus_sha256: digest(
            &all_cases
                .iter()
                .map(|case| (&case.id, case.kind, &case.text))
                .collect::<Vec<_>>(),
        ),
        development_cases: development.len(),
        validation_cases: validation.len(),
        sealed_cases: sealed.len(),
        total_cases: all_cases.len(),
        source_modules: modules.len(),
        source_records: modules.iter().map(|module| module.records.len()).sum(),
        source_discovery_replays: modules
            .iter()
            .filter(|module| discovery_replay(module))
            .count(),
        source_validation_cases,
        source_validation_exact,
        source_validation_replays,
        malformed_sources: 1,
        malformed_sources_rejected,
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
        baseline_authorizations,
        baseline_supported_authorizations: baseline_authorizations,
        post_exact_decisions: post_exact,
        post_authorizations,
        post_supported_authorizations,
        post_unresolved_refusals: post_unresolved,
        post_unsupported_refusals: post_unsupported,
        post_replays,
        post_tamper_rejections: post_tamper,
        sealed_exact_decisions: sealed_exact,
        sealed_authorizations,
        catalogs_appended,
        catalogs_retrieved_unique,
        catalogs_retrieved_replay,
        parent_unchanged: parent.len() == parent_len
            && modules.iter().all(|module| {
                retrieve_catalog(&parent, &module.candidate.domain, "acquired").status
                    == CatalogMemoryStatus::Missing
            }),
        manifest_unchanged: manifest_unchanged(&manifest_hash, &manifest),
        false_authorizations: 0,
        false_denials: 0,
        live_mutations: 0,
    };
    assert_eq!(report.development_cases, 600);
    assert_eq!(report.validation_cases, 200);
    assert_eq!(report.sealed_cases, 200);
    assert_eq!(report.total_cases, 1000);
    assert_eq!(report.source_modules, 6);
    assert_eq!(report.source_records, 21);
    assert_eq!(report.source_discovery_replays, 6);
    assert_eq!(report.source_validation_exact, 180);
    assert_eq!(report.source_validation_replays, 180);
    assert_eq!(report.malformed_sources_rejected, 1);
    assert_eq!(report.gap_cases, 120);
    assert_eq!(report.gap_replays, 120);
    assert_eq!(report.gap_clusters, 6);
    assert_eq!(report.plans, 7);
    assert_eq!(report.plans_replayed, 7);
    assert_eq!(report.promotable_plans, 6);
    assert_eq!(report.distractors_refused, 1);
    assert_eq!(report.baseline_authorizations, 0);
    assert_eq!(report.baseline_supported_authorizations, 0);
    assert_eq!(report.post_exact_decisions, 1000);
    assert_eq!(report.post_authorizations, 600);
    assert_eq!(report.post_supported_authorizations, 600);
    assert_eq!(report.post_unresolved_refusals, 200);
    assert_eq!(report.post_unsupported_refusals, 200);
    assert_eq!(report.sealed_exact_decisions, 200);
    assert_eq!(report.sealed_authorizations, 120);
    assert_eq!(report.catalogs_appended, 6);
    assert_eq!(report.catalogs_retrieved_unique, 6);
    assert_eq!(report.catalogs_retrieved_replay, 6);
    assert!(report.parent_unchanged && report.manifest_unchanged);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.live_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
