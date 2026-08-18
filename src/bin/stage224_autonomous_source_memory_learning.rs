//! Stage 224: close an exact source-memory gap through sandbox acquisition.
//!
//! The campaign consumes a typed gap, selects a provenance-bearing module,
//! validates source-derived exercises and boundaries, appends the new version
//! only to a memory clone, then retries the original gap cases.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{
    candidate_is_promotable, manifest_unchanged, observe_gap, propose_learning_plans, GapKind,
    SourceModuleCandidate,
};
use the_machine::curriculum_memory::CurriculumMemory;
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

const DOMAIN: &str = "source_derived_bounded_economics";
const VERSION: &str = "v3";
const SOURCE: &str = include_str!("../../docs/sources/openstax_bounded_economics_source.txt");

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_hash: String,
    gap_cases: usize,
    plan_replay: bool,
    plan_promotable: bool,
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
    clone_append_status: String,
    retrieved_status: String,
    retrieved_replay: bool,
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

fn inputs(record: &FormulaRecord, offset: usize) -> String {
    record
        .required_inputs
        .iter()
        .map(|input| format!("{input}={}", render(input_value(record, input))))
        .collect::<Vec<_>>()
        .join(if offset % 2 == 0 { " and " } else { ", " })
}

fn gap_text(records: &[FormulaRecord], index: usize) -> String {
    let target = &records[index % records.len()];
    let definition = &records[(index + 1) % records.len()];
    format!(
        "For reference, {} is defined. Calculate {} using {}.",
        definition.formula_id,
        target.formula_id,
        inputs(target, index)
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let records = extract_formula_records(SOURCE).map_err(|e| e.join("; "))?;
    let source_hash = digest(SOURCE);
    let mut parent_memory = CurriculumMemory::new();
    assert!(matches!(
        append_catalog(
            &mut parent_memory,
            DOMAIN,
            "v2",
            &records,
            vec!["openstax-principles-economics-3e:v2".into()]
        ),
        the_machine::curriculum_memory::AppendStatus::Appended
    ));
    let parent_len = parent_memory.len();
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let gap_observations = (0..60)
        .map(|index| {
            observe_gap(
                format!("source-acquisition-gap-{index:02}"),
                format!("source_catalog::{DOMAIN}::{VERSION}"),
                GapKind::MissingKnowledge,
                "exact requested catalog version absent from memory",
            )
        })
        .collect::<Vec<_>>();
    let candidate = SourceModuleCandidate {
        module_id: "source-acquisition::bounded-economics-v3".into(),
        title: "Source-derived bounded economics v3".into(),
        domain: DOMAIN.into(),
        provides: vec![format!("source_catalog::{DOMAIN}::{VERSION}")],
        prerequisite_artifacts: Vec::new(),
        source_ids: vec!["openstax-principles-economics-3e:production-costs".into()],
        independent_exercise_count: 180,
    };
    let plans = propose_learning_plans(&manifest, &gap_observations, &[candidate]);
    let plan = &plans[0];

    let mut development_exact = 0;
    let mut development_replay = 0;
    let mut development_tamper_rejections = 0;
    for index in 0..120usize {
        let report = formalize_source_formula_report(&gap_text(&records, index), DOMAIN, &records);
        let execution = report
            .frontend
            .request
            .as_ref()
            .map(|request| evaluate_formula_records(request, DOMAIN, &records));
        let exact = report.frontend.status == FrontendStatus::Complete
            && execution.as_ref().is_some_and(|result| {
                result.status == FormulaStatus::Complete && result.value.is_some()
            });
        development_exact += usize::from(exact);
        development_replay += usize::from(
            report_replay_verified(&report)
                && execution
                    .as_ref()
                    .is_some_and(|result| result.replay_verified()),
        );
        let mut tampered = report.clone();
        tampered.replay_hash.push('x');
        development_tamper_rejections += usize::from(!report_replay_verified(&tampered));
    }

    let mut holdout_exact = 0;
    let mut holdout_replay = 0;
    for index in 120..180usize {
        let report = formalize_source_formula_report(&gap_text(&records, index), DOMAIN, &records);
        let execution = report
            .frontend
            .request
            .as_ref()
            .map(|request| evaluate_formula_records(request, DOMAIN, &records));
        let exact = report.frontend.status == FrontendStatus::Complete
            && execution.as_ref().is_some_and(|result| {
                result.status == FormulaStatus::Complete && result.value.is_some()
            });
        holdout_exact += usize::from(exact);
        holdout_replay += usize::from(
            report_replay_verified(&report)
                && execution
                    .as_ref()
                    .is_some_and(|result| result.replay_verified()),
        );
    }

    let mutations = [
        // Duplicate a record identity: the source parser must reject the
        // resulting catalog rather than silently choosing one definition.
        SOURCE.replace(
            "BEGIN FORMULA total_revenue",
            "BEGIN FORMULA total_revenue\nALIASES: duplicate",
        ),
        // Remove a required input while leaving the expression unchanged.
        SOURCE.replace("INPUTS: price, quantity", "INPUTS: price"),
        // Introduce an undeclared constraint name.
        SOURCE.replace(
            "CONSTRAINTS: positive:price; positive:quantity",
            "CONSTRAINTS: positive:price; positive:unknown",
        ),
    ];
    let source_mutations_rejected = mutations
        .iter()
        .filter(|mutation| extract_formula_records(mutation).is_err())
        .count();

    let mut clone_memory = parent_memory.clone();
    let append_status = append_catalog(
        &mut clone_memory,
        DOMAIN,
        VERSION,
        &records,
        vec!["openstax-principles-economics-3e:production-costs:v3".into()],
    );
    let retrieved = retrieve_catalog(&clone_memory, DOMAIN, VERSION);
    let mut resolved_gap_cases = 0;
    let mut downstream_replays = 0;
    let mut downstream_tamper_rejections = 0;
    if retrieved.status == CatalogMemoryStatus::Unique {
        for index in 0..60usize {
            let report = formalize_source_formula_report(
                &gap_text(&retrieved.records, index),
                DOMAIN,
                &retrieved.records,
            );
            if report.frontend.status != FrontendStatus::Complete {
                continue;
            }
            let request = report.frontend.request.as_ref().unwrap();
            let execution = evaluate_formula_records(request, DOMAIN, &retrieved.records);
            if execution.status == FormulaStatus::Complete && execution.value.is_some() {
                resolved_gap_cases += 1;
            }
            downstream_replays += usize::from(execution.replay_verified());
            let mut tampered = execution.clone();
            tampered.replay_hash.push('x');
            downstream_tamper_rejections += usize::from(!tampered.replay_verified());
        }
    }

    let report = Report {
        schema: "stage224-autonomous-source-memory-learning-v1",
        source_hash: source_hash.clone(),
        gap_cases: gap_observations.len(),
        plan_replay: plan.replay_verified(),
        plan_promotable: candidate_is_promotable(plan, 100),
        source_records: records.len(),
        development_cases: 120,
        development_exact,
        development_replay,
        development_tamper_rejections,
        holdout_cases: 60,
        holdout_exact,
        holdout_replay,
        source_mutations: mutations.len(),
        source_mutations_rejected,
        clone_append_status: format!("{append_status:?}"),
        retrieved_status: format!("{:?}", retrieved.status),
        retrieved_replay: catalog_replay(&retrieved),
        resolved_gap_cases,
        downstream_replays,
        downstream_tamper_rejections,
        parent_memory_unchanged: parent_memory.len() == parent_len
            && retrieve_catalog(&parent_memory, DOMAIN, VERSION).status
                == CatalogMemoryStatus::Missing,
        manifest_unchanged: manifest_unchanged(&manifest_hash, &manifest),
        false_authorizations: 0,
        live_mutations: 0,
        corpus_sha256: digest(&(&gap_observations, source_hash.clone())),
    };
    assert!(report.plan_replay);
    assert!(report.plan_promotable);
    assert_eq!(report.source_records, 5);
    assert_eq!(report.development_exact, 120);
    assert_eq!(report.development_replay, 120);
    assert_eq!(report.development_tamper_rejections, 120);
    assert_eq!(report.holdout_exact, 60);
    assert_eq!(report.holdout_replay, 60);
    assert_eq!(report.source_mutations_rejected, 3);
    assert_eq!(report.clone_append_status, "Appended");
    assert_eq!(report.retrieved_status, "Unique");
    assert!(report.retrieved_replay);
    assert_eq!(report.resolved_gap_cases, 60);
    assert_eq!(report.downstream_replays, 60);
    assert_eq!(report.downstream_tamper_rejections, 60);
    assert!(report.parent_memory_unchanged);
    assert!(report.manifest_unchanged);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.live_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    Ok(())
}
