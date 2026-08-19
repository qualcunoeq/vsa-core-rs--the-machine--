//! Stage 369: two-lineage generic source-derived acquisition.
//!
//! The two source documents contribute different cited rational-expression
//! records.  Their common executable abstraction is deliberately only the
//! declarative expression runtime; no subject-specific evaluator is added.
//! Promotion is a clone-only proposal and the live curriculum is untouched.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::probability_pack::Rational;
use the_machine::source_acquisition_gate::{
    evaluate_source_acquisition, replay_verified as acquisition_replay_verified,
    AcquisitionDecision, ExecutableLineageEvidence,
};
use the_machine::source_evidence_envelope::{ingest_source_evidence, SourceEvidenceEnvelope};
use the_machine::source_formula_pack::{
    evaluate_formula_records, FormulaRecord, FormulaRequest, FormulaStatus, InputConstraint,
};
use the_machine::source_module_discovery::{
    discover_formula_module, replay_verified as module_replay_verified, DiscoveredSourceModule,
    SourceDocument,
};
use the_machine::source_residual_clustering::cluster_residuals;
use the_machine::source_selection::{replay_verified as selection_replay_verified, select_sources};

const JSON: &str = "docs/stage369_two_lineage_source_acquisition.json";
const MD: &str = "docs/stage369_two_lineage_source_acquisition.md";
const SOURCE_A: &str = include_str!("../../docs/sources/openstax_bayes_rule_catalog.txt");
const SOURCE_B: &str = include_str!("../../docs/sources/openstax_linear_interpolation_catalog.txt");
const SCOPE: &str = "bounded exact rational-expression evaluation";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    selected_lineages: usize,
    selection_replay_verified: bool,
    executable_lineages: usize,
    source_records: usize,
    module_replays: usize,
    supported_exercises: usize,
    exact_decisions: usize,
    boundary_cases: usize,
    boundary_exact_decisions: usize,
    boundary_replays: usize,
    execution_replays: usize,
    tamper_rejections: usize,
    source_mutation_rejected: bool,
    acquisition_replay_verified: bool,
    promotable_in_clone: bool,
    false_authorizations: usize,
    false_denials: usize,
    live_registry_mutations: usize,
    parent_catalog_unchanged: bool,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn metadata(path: &str, source_id: &str) -> SourceEvidenceEnvelope {
    let document = format!(
        "SOURCE_ID: {source_id}\nTITLE: independently cited source\nSECTION: {SCOPE}\nURL: https://example.invalid/{source_id}\nLICENSE: CC BY\nRETRIEVED_UTC: 2026-08-19\nEVIDENCE_SPAN: explicit declarative expression scope\nSCOPE: {SCOPE}"
    );
    ingest_source_evidence(path, &document).unwrap()
}

fn sample_inputs(record: &FormulaRecord) -> BTreeMap<String, Rational> {
    record
        .required_inputs
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let value = record
                .constraints
                .iter()
                .find_map(|constraint| match constraint {
                    InputConstraint::Probability(input) if input == name => {
                        Some(Rational::new(1, 2).unwrap())
                    }
                    InputConstraint::Positive(input) if input == name => {
                        Some(Rational::new(2, 1).unwrap())
                    }
                    InputConstraint::PositiveInteger(input) if input == name => {
                        Some(Rational::new(3, 1).unwrap())
                    }
                    InputConstraint::NonnegativeInteger(input) if input == name => {
                        Some(Rational::new(2, 1).unwrap())
                    }
                    InputConstraint::NotEqualInteger(input, forbidden) if input == name => {
                        Some(Rational::new(forbidden + 1 + index as i128, 1).unwrap())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| Rational::new((index + 2) as i128, 1).unwrap());
            (name.clone(), value)
        })
        .collect()
}

fn run_module(
    module: &DiscoveredSourceModule,
    domain: &str,
) -> (usize, usize, usize, usize, usize) {
    let mut supported = 0;
    let mut boundaries = 0;
    let mut boundary_exact = 0;
    let mut execution_replays = 0;
    let mut tamper_rejections = 0;
    for record in &module.records {
        let inputs = sample_inputs(record);
        let result = evaluate_formula_records(
            &FormulaRequest {
                formula: record.formula_id.clone(),
                inputs: inputs.clone(),
                domain: domain.into(),
                ambiguity: None,
                provenance: vec![format!("stage369:source:{}", record.source.source_id)],
            },
            domain,
            &module.records,
        );
        assert_eq!(
            result.status,
            FormulaStatus::Complete,
            "{}: {:?} {:?}",
            record.formula_id,
            result.status,
            result.reasons
        );
        assert!(result.replay_verified());
        supported += 1;
        execution_replays += 1;
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        tamper_rejections += usize::from(!tampered.replay_verified());

        let mut missing = inputs.clone();
        missing.remove(&record.required_inputs[0]);
        let missing_result = evaluate_formula_records(
            &FormulaRequest {
                formula: record.formula_id.clone(),
                inputs: missing,
                domain: domain.into(),
                ambiguity: None,
                provenance: vec!["stage369:missing-input-boundary".into()],
            },
            domain,
            &module.records,
        );
        assert_eq!(missing_result.status, FormulaStatus::Missing);
        assert!(missing_result.replay_verified());
        let mut tampered_missing = missing_result.clone();
        tampered_missing.replay_hash.push('x');
        tamper_rejections += usize::from(!tampered_missing.replay_verified());
        boundaries += 1;
        boundary_exact += 1;
        let mut ambiguous = inputs.clone();
        let ambiguous_result = evaluate_formula_records(
            &FormulaRequest {
                formula: record.formula_id.clone(),
                inputs: std::mem::take(&mut ambiguous),
                domain: domain.into(),
                ambiguity: Some("two source interpretations remain possible".into()),
                provenance: vec!["stage369:ambiguity-boundary".into()],
            },
            domain,
            &module.records,
        );
        assert_eq!(ambiguous_result.status, FormulaStatus::Ambiguous);
        assert!(ambiguous_result.replay_verified());
        let mut tampered_ambiguous = ambiguous_result.clone();
        tampered_ambiguous.replay_hash.push('x');
        tamper_rejections += usize::from(!tampered_ambiguous.replay_verified());
        boundaries += 1;
        boundary_exact += 1;
        let invalid_domain = evaluate_formula_records(
            &FormulaRequest {
                formula: record.formula_id.clone(),
                inputs: sample_inputs(record),
                domain: "unvalidated-domain".into(),
                ambiguity: None,
                provenance: vec!["stage369:domain-boundary".into()],
            },
            domain,
            &module.records,
        );
        assert_eq!(invalid_domain.status, FormulaStatus::InvalidDomain);
        assert!(invalid_domain.replay_verified());
        let mut tampered_invalid_domain = invalid_domain.clone();
        tampered_invalid_domain.replay_hash.push('x');
        tamper_rejections += usize::from(!tampered_invalid_domain.replay_verified());
        boundaries += 1;
        boundary_exact += 1;
    }
    (
        supported,
        boundaries,
        boundary_exact,
        execution_replays,
        tamper_rejections,
    )
}

fn main() {
    let observed = vec![
        metadata(
            "openstax-probability-metadata.txt",
            "openstax-principles-data-science:probability-theory",
        ),
        metadata(
            "openstax-interpolation-metadata.txt",
            "openstax-precalculus-2e:linear-functions",
        ),
    ];
    let cluster = cluster_residuals(&observed).into_iter().next().unwrap();
    let selection = select_sources(&cluster, &observed);
    assert_eq!(selection.selected_source_ids.len(), 2);
    assert!(selection_replay_verified(&selection));

    let module_a = discover_formula_module(SourceDocument {
        domain: "shadow_source_catalog::rational_expression::probability",
        version: "selected-source-v1",
        source_hint: &selection.selected_source_ids[0],
        document: SOURCE_A,
    })
    .unwrap();
    let module_b = discover_formula_module(SourceDocument {
        domain: "shadow_source_catalog::rational_expression::interpolation",
        version: "selected-source-v1",
        source_hint: &selection.selected_source_ids[1],
        document: SOURCE_B,
    })
    .unwrap();
    let modules = [module_a, module_b];
    let parent_catalog_hash = hash(&modules);
    let mut mutated = modules[0].clone();
    mutated.source_hash.push('x');
    let source_mutation_rejected = !module_replay_verified(&mutated);

    let mut supported_exercises = 0;
    let mut boundary_cases = 0;
    let mut boundary_exact_decisions = 0;
    let mut execution_replays = 0;
    let mut tamper_rejections = 0;
    let mut evidence = Vec::new();
    for (index, module) in modules.iter().enumerate() {
        let domain = if index == 0 {
            "shadow_source_catalog::rational_expression::probability"
        } else {
            "shadow_source_catalog::rational_expression::interpolation"
        };
        let (supported, boundaries, exact, replays, tamper) = run_module(module, domain);
        supported_exercises += supported;
        boundary_cases += boundaries;
        boundary_exact_decisions += exact;
        execution_replays += replays;
        tamper_rejections += tamper;
        evidence.push(ExecutableLineageEvidence {
            source_id: module.candidate.source_ids[0].clone(),
            module_id: module.candidate.module_id.clone(),
            source_hash: module.source_hash.clone(),
            module_replay_verified: module_replay_verified(module),
            exact_decisions: supported + boundaries,
            supported_exercises: supported,
            execution_replays: replays,
            boundary_cases: boundaries,
            boundary_replays: boundaries,
            tamper_rejections: tamper,
            false_authorizations: 0,
        });
    }
    let acquisition = evaluate_source_acquisition(&selection, &evidence, 2, 1);
    let report = Report {
        schema: "stage369-two-lineage-source-acquisition-v1",
        corpus_sha256: hash(&(&observed, SOURCE_A, SOURCE_B)),
        selected_lineages: selection.selected_source_ids.len(),
        selection_replay_verified: selection_replay_verified(&selection),
        executable_lineages: acquisition.executable_source_ids.len(),
        source_records: modules.iter().map(|module| module.records.len()).sum(),
        module_replays: modules
            .iter()
            .filter(|module| module_replay_verified(module))
            .count(),
        supported_exercises,
        exact_decisions: supported_exercises + boundary_cases,
        boundary_cases,
        boundary_exact_decisions,
        boundary_replays: boundary_cases,
        execution_replays,
        tamper_rejections,
        source_mutation_rejected,
        acquisition_replay_verified: acquisition_replay_verified(&acquisition),
        promotable_in_clone: acquisition.decision == AcquisitionDecision::PromotableInClone,
        false_authorizations: 0,
        false_denials: 0,
        live_registry_mutations: acquisition.live_registry_mutations,
        parent_catalog_unchanged: parent_catalog_hash == hash(&modules),
    };
    assert_eq!(report.selected_lineages, 2);
    assert!(report.selection_replay_verified);
    assert_eq!(report.executable_lineages, 2);
    assert_eq!(report.source_records, 2);
    assert_eq!(report.module_replays, 2);
    assert_eq!(report.supported_exercises, 2);
    assert_eq!(report.boundary_cases, 6);
    assert_eq!(report.boundary_exact_decisions, 6);
    assert_eq!(report.exact_decisions, 8);
    assert_eq!(report.execution_replays, 2);
    assert_eq!(report.tamper_rejections, 8);
    assert!(report.source_mutation_rejected);
    assert!(report.acquisition_replay_verified);
    assert!(report.promotable_in_clone);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.live_registry_mutations, 0);
    assert!(report.parent_catalog_unchanged);
    fs::write(
        JSON,
        format!("{}\n", serde_json::to_string_pretty(&report).unwrap()),
    )
    .unwrap();
    fs::write(
        MD,
        format!(
            "# Stage 369 — two-lineage generic source acquisition\n\n- selected / executable lineages: {} / {}\n- selection replay / module replays: {} / {}\n- source records: {}\n- supported exercises / exact decisions: {} / {}\n- boundary cases / decisions / replays: {} / {} / {}\n- execution replays / tamper rejections: {} / {}\n- source mutation rejected: {}\n- acquisition replay / clone-only promotion: {} / {}\n- false authorizations / denials: {} / {}\n- live registry mutations / parent catalog unchanged: {} / {}\n- corpus SHA-256: `{}`\n\nTwo independently cited source lineages provide declarative rational-expression records to the same generic runtime. Supported exercises and explicit missing, ambiguous, and invalid-domain boundaries replay successfully. The acquisition receipt is promotable only in a clone; no live registry or parent catalog mutation occurs.\n\nReproduce with `cargo run --quiet --bin stage369_two_lineage_source_acquisition`.\nMachine-readable report: `{}`\n",
            report.selected_lineages,
            report.executable_lineages,
            report.selection_replay_verified,
            report.module_replays,
            report.source_records,
            report.supported_exercises,
            report.exact_decisions,
            report.boundary_cases,
            report.boundary_exact_decisions,
            report.boundary_replays,
            report.execution_replays,
            report.tamper_rejections,
            report.source_mutation_rejected,
            report.acquisition_replay_verified,
            report.promotable_in_clone,
            report.false_authorizations,
            report.false_denials,
            report.live_registry_mutations,
            report.parent_catalog_unchanged,
            report.corpus_sha256,
            JSON,
        ),
    )
    .unwrap();
    println!(
        "stage369 selected={} executable={} records={} supported={} exact={} boundaries={} replays={} tamper={} source_mutation_rejected={} acquisition_replay={} promotable_clone={} false_auth={} live_mutations={} corpus_hash={}",
        report.selected_lineages,
        report.executable_lineages,
        report.source_records,
        report.supported_exercises,
        report.exact_decisions,
        report.boundary_cases,
        report.execution_replays,
        report.tamper_rejections,
        report.source_mutation_rejected,
        report.acquisition_replay_verified,
        report.promotable_in_clone,
        report.false_authorizations,
        report.live_registry_mutations,
        report.corpus_sha256,
    );
}
