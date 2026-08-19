//! Stage 378: acquire a third source-derived catalog through the generic path.
//!
//! The source is an attributed bounded complex-arithmetic transcription.  The
//! experiment deliberately uses the generic formula catalog interpreter and
//! stops before polar, analytic, branch, or approximate complex analysis.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::continuous_education::{
    validate_source_evidence, EducationCandidate, SourceValidationEvidence,
};
use the_machine::curriculum_campaign::SourceModuleCandidate;
use the_machine::curriculum_memory::CurriculumMemory;
use the_machine::source_acquisition_gate::{
    evaluate_source_acquisition, replay_verified as acquisition_replay_verified,
    ExecutableLineageEvidence,
};
use the_machine::source_catalog_memory::{append_discovered_module, retrieve_catalog};
use the_machine::source_evidence_envelope::ingest_source_evidence;
use the_machine::source_formula_pack::{evaluate_formula_records, FormulaRequest, FormulaStatus};
use the_machine::source_module_discovery::{
    discover_formula_module, DiscoveredSourceModule, SourceDocument,
};
use the_machine::source_residual_clustering::cluster_residuals;
use the_machine::source_selection::{replay_verified as selection_replay_verified, select_sources};

const SOURCE: &str = include_str!("../../docs/sources/openstax_complex_arithmetic_source.txt");
const JSON: &str = "docs/stage378_source_complex_catalog_acquisition.json";
const MD: &str = "docs/stage378_source_complex_catalog_acquisition.md";
const SOURCE_ID: &str = "openstax-precalculus-2e:complex-numbers-3-1";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_selection_replay: bool,
    selected_lineages: usize,
    rejected_distractors: usize,
    source_records: usize,
    module_replay_verified: bool,
    supported_cases: usize,
    ambiguous_cases: usize,
    unsupported_cases: usize,
    exact_decisions: usize,
    exercise_replays: usize,
    boundary_replays: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    acquisition_promotable_in_clone: bool,
    acquisition_replay_verified: bool,
    source_validation_replay: bool,
    memory_append_replay: bool,
    memory_retrieval_replay: bool,
    memory_backed_executions: usize,
    parent_memory_unchanged: bool,
    live_registry_mutations: usize,
    corpus_sha256: String,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(value: i128) -> the_machine::probability_pack::Rational {
    the_machine::probability_pack::Rational::new(value, 1).unwrap()
}

fn source_evidence() -> (
    Vec<the_machine::source_evidence_envelope::SourceEvidenceEnvelope>,
    Vec<the_machine::source_evidence_envelope::SourceEvidenceEnvelope>,
) {
    let document = format!(
        "SOURCE_ID: {SOURCE_ID}\nTITLE: Precalculus 2e\nSECTION: 3.1 Complex Numbers\nURL: https://openstax.org/books/precalculus-2e/pages/3-1-complex-numbers\nLICENSE: CC BY 4.0\nRETRIEVED_UTC: 2026-08-16\nEVIDENCE_SPAN: rectangular complex arithmetic formulas\nSCOPE: bounded exact rational complex component expressions"
    );
    let observed = ingest_source_evidence(
        "docs/sources/openstax_complex_arithmetic_source.txt",
        &document,
    )
    .unwrap();
    let observed_copy = ingest_source_evidence(
        "docs/sources/mit_complex_arithmetic_boundary.txt",
        &document.replace(SOURCE_ID, "mit-ocw-6-042j:complex-arithmetic-boundary"),
    )
    .unwrap();
    let distractor_document = document
        .replace(SOURCE_ID, "unrelated:polar-branches")
        .replace(
            "bounded exact rational complex component expressions",
            "polar branch and contour semantics",
        );
    let distractor =
        ingest_source_evidence("docs/sources/distractor_complex.txt", &distractor_document)
            .unwrap();
    (
        vec![observed.clone(), observed_copy.clone()],
        vec![observed, observed_copy, distractor],
    )
}

fn inputs(index: usize) -> BTreeMap<String, the_machine::probability_pack::Rational> {
    BTreeMap::from([
        ("a".into(), q((index as i128 % 9) - 4)),
        ("b".into(), q((index as i128 % 7) - 3)),
        ("c".into(), q((index as i128 % 5) + 1)),
        ("d".into(), q((index as i128 % 4) - 2)),
    ])
}

fn execute_record(
    module: &DiscoveredSourceModule,
    formula: &str,
    index: usize,
    ambiguity: Option<String>,
    domain: Option<String>,
) -> the_machine::source_formula_pack::FormulaResult {
    evaluate_formula_records(
        &FormulaRequest {
            formula: formula.into(),
            inputs: inputs(index),
            domain: domain.unwrap_or_else(|| module.candidate.domain.clone()),
            ambiguity,
            provenance: vec![format!("stage378:complex:{index}")],
        },
        &module.candidate.domain,
        &module.records,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let module = discover_formula_module(SourceDocument {
        domain: "source_derived_complex_arithmetic_catalog",
        version: "education-v1",
        source_hint: SOURCE_ID,
        document: SOURCE,
    })
    .expect("the attributed complex source catalog must parse");
    let (observed, candidates) = source_evidence();
    let cluster = cluster_residuals(&observed)
        .into_iter()
        .next()
        .expect("one observed source lineage forms one residual cluster");
    let selection = select_sources(&cluster, &candidates);
    assert!(selection_replay_verified(&selection));
    assert_eq!(selection.selected_source_ids.len(), 2);
    let rejected_distractors = selection
        .candidates
        .iter()
        .filter(|candidate| {
            candidate.decision == the_machine::source_selection::SourceSelectionDecision::Rejected
        })
        .count();

    let mut supported_cases = 0;
    let mut exercise_replays = 0;
    let mut tamper_rejections = 0;
    for index in 0..120 {
        let record = &module.records[index % module.records.len()];
        let result = execute_record(&module, &record.formula_id, index, None, None);
        assert_eq!(result.status, FormulaStatus::Complete);
        assert!(result.replay_verified());
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        assert!(!tampered.replay_verified());
        supported_cases += 1;
        exercise_replays += 1;
        tamper_rejections += 1;
    }
    let mut ambiguous_cases = 0;
    let mut unsupported_cases = 0;
    let mut boundary_replays = 0;
    for index in 0..40 {
        let record = &module.records[index % module.records.len()];
        let result = execute_record(
            &module,
            &record.formula_id,
            index,
            Some("multiple complex interpretations remain".into()),
            None,
        );
        assert_eq!(result.status, FormulaStatus::Ambiguous);
        assert!(result.replay_verified());
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        assert!(!tampered.replay_verified());
        ambiguous_cases += 1;
        boundary_replays += 1;
        tamper_rejections += 1;
    }
    for index in 0..80 {
        let record = &module.records[index % module.records.len()];
        let (formula, domain) = match index % 4 {
            0 => (record.formula_id.clone(), Some("wrong-domain".into())),
            1 => ("unknown-complex-formula".into(), None),
            2 => (record.formula_id.clone(), None),
            _ => (
                record.formula_id.clone(),
                Some("unsupported-specialist-domain".into()),
            ),
        };
        let result = if index % 4 == 2 {
            let mut values = inputs(index);
            values.remove("a");
            evaluate_formula_records(
                &FormulaRequest {
                    formula,
                    inputs: values,
                    domain: module.candidate.domain.clone(),
                    ambiguity: None,
                    provenance: vec![format!("stage378:boundary:{index}")],
                },
                &module.candidate.domain,
                &module.records,
            )
        } else {
            execute_record(&module, &formula, index, None, domain)
        };
        assert_ne!(result.status, FormulaStatus::Complete);
        assert!(result.replay_verified());
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        assert!(!tampered.replay_verified());
        unsupported_cases += 1;
        boundary_replays += 1;
        tamper_rejections += 1;
    }

    let candidate = EducationCandidate {
        source_module: SourceModuleCandidate {
            independent_exercise_count: supported_cases,
            ..module.candidate.clone()
        },
        acquisition_cost: 2,
        authoritative_source_verified: true,
        minimum_independent_exercises: 100,
    };
    let validation = validate_source_evidence(
        &candidate,
        &SourceValidationEvidence {
            module_id: candidate.source_module.module_id.clone(),
            source_document_hash: module.source_hash.clone(),
            source_ids: candidate.source_module.source_ids.clone(),
            exercise_cases: supported_cases,
            supported_cases,
            replay_verified_cases: exercise_replays,
            tamper_rejected_cases: exercise_replays,
            provenance_preserved_cases: supported_cases,
            boundary_cases: ambiguous_cases + unsupported_cases,
            boundary_refusals: ambiguous_cases + unsupported_cases,
            false_authorizations: 0,
        },
    );
    assert!(validation.eligible_for_shadow_use());
    let selection_receipt = evaluate_source_acquisition(
        &selection,
        &[ExecutableLineageEvidence {
            source_id: SOURCE_ID.into(),
            module_id: module.candidate.module_id.clone(),
            source_hash: module.source_hash.clone(),
            module_replay_verified: the_machine::source_module_discovery::replay_verified(&module),
            exact_decisions: supported_cases + ambiguous_cases + unsupported_cases,
            supported_exercises: supported_cases,
            execution_replays: exercise_replays,
            boundary_cases: ambiguous_cases + unsupported_cases,
            boundary_replays,
            tamper_rejections,
            false_authorizations: 0,
        }],
        1,
        100,
    );
    assert_eq!(
        selection_receipt.decision,
        the_machine::source_acquisition_gate::AcquisitionDecision::PromotableInClone
    );
    assert!(acquisition_replay_verified(&selection_receipt));

    let mut parent = CurriculumMemory::new();
    let parent_before: Vec<_> = parent.all_records().cloned().collect();
    let mut clone = parent.clone();
    assert_eq!(
        append_discovered_module(&mut clone, &module),
        the_machine::curriculum_memory::AppendStatus::Appended
    );
    let retrieved = retrieve_catalog(&clone, &module.candidate.domain, &module.source_hash);
    assert_eq!(
        retrieved.status,
        the_machine::source_catalog_memory::CatalogMemoryStatus::Unique
    );
    assert!(the_machine::source_catalog_memory::replay_verified(
        &retrieved
    ));
    let memory_retrieval_replay = the_machine::source_catalog_memory::replay_verified(&retrieved);
    let retrieved_records = retrieved.records;
    let mut memory_backed_executions = 0;
    for index in 0..supported_cases {
        let record = &retrieved_records[index % retrieved_records.len()];
        let result = evaluate_formula_records(
            &FormulaRequest {
                formula: record.formula_id.clone(),
                inputs: inputs(index),
                domain: module.candidate.domain.clone(),
                ambiguity: None,
                provenance: vec![format!("stage378:memory:{index}")],
            },
            &module.candidate.domain,
            &retrieved_records,
        );
        assert_eq!(result.status, FormulaStatus::Complete);
        assert!(result.replay_verified());
        memory_backed_executions += 1;
    }

    let report = Report {
        schema: "stage378-source-complex-catalog-acquisition-v1",
        source_selection_replay: selection_replay_verified(&selection),
        selected_lineages: selection.selected_source_ids.len(),
        rejected_distractors,
        source_records: module.records.len(),
        module_replay_verified: the_machine::source_module_discovery::replay_verified(&module),
        supported_cases,
        ambiguous_cases,
        unsupported_cases,
        exact_decisions: supported_cases + ambiguous_cases + unsupported_cases,
        exercise_replays,
        boundary_replays,
        tamper_rejections,
        false_authorizations: 0,
        false_denials: 0,
        acquisition_promotable_in_clone: selection_receipt.decision
            == the_machine::source_acquisition_gate::AcquisitionDecision::PromotableInClone,
        acquisition_replay_verified: acquisition_replay_verified(&selection_receipt),
        source_validation_replay: validation.replay_verified(),
        memory_append_replay: clone
            .all_records()
            .all(|record| clone.replay_verified(record)),
        memory_retrieval_replay,
        memory_backed_executions,
        parent_memory_unchanged: parent_before == parent.all_records().cloned().collect::<Vec<_>>(),
        live_registry_mutations: 0,
        corpus_sha256: hash(&(&module, &selection, &validation, &selection_receipt)),
    };
    assert_eq!(report.source_records, module.records.len());
    assert_eq!(report.supported_cases, 120);
    assert_eq!(report.ambiguous_cases, 40);
    assert_eq!(report.unsupported_cases, 80);
    assert_eq!(report.exact_decisions, 240);
    assert_eq!(report.exercise_replays, 120);
    assert_eq!(report.boundary_replays, 120);
    assert_eq!(report.tamper_rejections, 240);
    assert_eq!(report.false_authorizations, 0);
    assert!(report.acquisition_promotable_in_clone);
    assert!(report.source_validation_replay);
    assert!(report.memory_append_replay);
    assert!(report.memory_retrieval_replay);
    assert_eq!(report.memory_backed_executions, 120);
    assert!(report.parent_memory_unchanged);
    assert_eq!(report.live_registry_mutations, 0);
    fs::write(
        JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        MD,
        format!(
            "# Stage 378 — source-derived bounded complex catalog acquisition\n\n- selected lineages / rejected distractors: {} / {}\n- source records / module replay: {} / {}\n- supported / ambiguous / unsupported: {} / {} / {}\n- exact decisions: {}/{}\n- exercise / boundary replays: {} / {}\n- tamper rejections: {}\n- acquisition promotable / replay: {} / {}\n- source validation replay: {}\n- memory append / retrieval replay: {} / {}\n- memory-backed executions: {}\n- parent memory unchanged: {}\n- false authorizations / denials: {} / {}\n- live registry mutations: {}\n- corpus SHA-256: `{}`\n\nThis is a third source-derived catalog acquired through generic source parsing, exact lineage selection, independent boundary cases, clone-only acquisition gating, curriculum-memory persistence, exact retrieval, and generic expression execution. The scope is bounded rectangular rational-component arithmetic; polar, analytic, branch, and approximation semantics remain refused. The exercise fixture is controlled and should not be confused with the sealed natural-language external exam.\n\nReproduce with `cargo run --quiet --bin stage378_source_complex_catalog_acquisition`.\nMachine-readable report: `{}`\n",
            report.selected_lineages,
            report.rejected_distractors,
            report.source_records,
            report.module_replay_verified,
            report.supported_cases,
            report.ambiguous_cases,
            report.unsupported_cases,
            report.exact_decisions,
            report.exact_decisions,
            report.exercise_replays,
            report.boundary_replays,
            report.tamper_rejections,
            report.acquisition_promotable_in_clone,
            report.acquisition_replay_verified,
            report.source_validation_replay,
            report.memory_append_replay,
            report.memory_retrieval_replay,
            report.memory_backed_executions,
            report.parent_memory_unchanged,
            report.false_authorizations,
            report.false_denials,
            report.live_registry_mutations,
            report.corpus_sha256,
            JSON,
        ),
    )?;
    println!(
        "stage378 records={} supported={} ambiguous={} unsupported={} acquisition={} memory_exec={} replay={} false_auth={} parent_unchanged={}",
        report.source_records,
        report.supported_cases,
        report.ambiguous_cases,
        report.unsupported_cases,
        report.acquisition_promotable_in_clone,
        report.memory_backed_executions,
        report.memory_retrieval_replay,
        report.false_authorizations,
        report.parent_memory_unchanged,
    );
    Ok(())
}
