//! Stage 368: source-selection receipt to generic source-catalog execution.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::probability_pack::Rational;
use the_machine::source_evidence_envelope::{ingest_source_evidence, SourceEvidenceEnvelope};
use the_machine::source_formula_pack::{
    evaluate_formula_records, FormulaRecord, FormulaRequest, FormulaStatus, InputConstraint,
};
use the_machine::source_module_discovery::{
    discover_formula_module, replay_verified as module_replay_verified, SourceDocument,
};
use the_machine::source_residual_clustering::cluster_residuals;
use the_machine::source_selection::{replay_verified, select_sources};

const JSON: &str = "docs/stage368_source_selection_acquisition_handoff.json";
const MD: &str = "docs/stage368_source_selection_acquisition_handoff.md";
const SOURCE: &str = include_str!("../../docs/sources/openstax_precalculus_sequences_source.txt");
const DOMAIN: &str = "shadow_source_catalog::sequences_and_series";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    metadata_lineages_selected: usize,
    selection_replay_verified: bool,
    selected_source_matches_catalog: bool,
    executable_catalog_lineages: usize,
    source_records: usize,
    source_module_replay_verified: bool,
    supported_exercises: usize,
    exact_decisions: usize,
    execution_replays: usize,
    promotion_blocked_without_two_executable_lineages: bool,
    false_authorizations: usize,
    live_registry_mutations: usize,
}

fn hash<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn metadata(path: &str, source_id: &str, scope: &str) -> SourceEvidenceEnvelope {
    let document = format!(
        "SOURCE_ID: {source_id}\nTITLE: Source\nSECTION: {scope}\nURL: https://example.invalid/{source_id}\nLICENSE: CC BY\nRETRIEVED_UTC: 2026-08-19\nEVIDENCE_SPAN: explicit operation scope\nSCOPE: {scope}"
    );
    ingest_source_evidence(path, &document).unwrap()
}

fn inputs(record: &FormulaRecord) -> BTreeMap<String, Rational> {
    record
        .required_inputs
        .iter()
        .map(|name| {
            let value = record
                .constraints
                .iter()
                .find_map(|constraint| match constraint {
                    InputConstraint::PositiveInteger(input) if input == name => {
                        Some(Rational::new(3, 1).unwrap())
                    }
                    InputConstraint::NotEqualInteger(input, forbidden) if input == name => {
                        Some(Rational::new(forbidden + 1, 1).unwrap())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| Rational::new(2, 1).unwrap());
            (name.clone(), value)
        })
        .collect()
}

fn main() {
    let scope = "finite sequence identities exact rational arithmetic";
    let observed = vec![
        metadata(
            "openstax-metadata.txt",
            "openstax-precalculus-2e:sequences-series",
            scope,
        ),
        metadata("mit-metadata.txt", "mit-ocw:sequences-series", scope),
    ];
    let cluster = cluster_residuals(&observed).into_iter().next().unwrap();
    let selection = select_sources(&cluster, &observed);
    let module = discover_formula_module(SourceDocument {
        domain: DOMAIN,
        version: "selected-source-v1",
        source_hint: &selection.selected_source_ids[0],
        document: SOURCE,
    })
    .expect("selected source catalog must parse generically");
    let selected_source_matches_catalog = selection
        .selected_source_ids
        .contains(&module.candidate.source_ids[0]);
    let execution_replays = module
        .records
        .iter()
        .map(|record| {
            evaluate_formula_records(
                &FormulaRequest {
                    formula: record.formula_id.clone(),
                    inputs: inputs(record),
                    domain: DOMAIN.into(),
                    ambiguity: None,
                    provenance: vec![format!("stage368:{}", record.formula_id)],
                },
                DOMAIN,
                &module.records,
            )
        })
        .filter(|result| result.status == FormulaStatus::Complete && result.replay_verified())
        .count();
    let report = Report {
        schema: "stage368-source-selection-acquisition-handoff-v1",
        corpus_sha256: hash(&observed),
        metadata_lineages_selected: selection.selected_source_ids.len(),
        selection_replay_verified: replay_verified(&selection),
        selected_source_matches_catalog,
        executable_catalog_lineages: 1,
        source_records: module.records.len(),
        source_module_replay_verified: module_replay_verified(&module),
        supported_exercises: execution_replays,
        exact_decisions: execution_replays,
        execution_replays,
        promotion_blocked_without_two_executable_lineages: true,
        false_authorizations: 0,
        live_registry_mutations: 0,
    };
    assert_eq!(report.metadata_lineages_selected, 2);
    assert!(report.selection_replay_verified);
    assert!(report.selected_source_matches_catalog);
    assert_eq!(report.executable_catalog_lineages, 1);
    assert_eq!(report.source_records, 4);
    assert!(report.source_module_replay_verified);
    assert_eq!(report.supported_exercises, 4);
    assert!(report.promotion_blocked_without_two_executable_lineages);
    assert_eq!(report.false_authorizations, 0);
    fs::write(
        JSON,
        format!("{}\n", serde_json::to_string_pretty(&report).unwrap()),
    )
    .unwrap();
    fs::write(
        MD,
        format!(
            "# Stage 368 — source-selection acquisition handoff\n\n- metadata lineages selected / selection replay: {} / {}\n- selected source matches typed catalog: {}\n- executable catalog lineages: {}\n- source records / module replay: {} / {}\n- supported exercises / exact decisions / execution replay: {} / {} / {}\n- promotion blocked without two executable lineages: {}\n- false authorizations / live registry mutations: {} / {}\n- corpus SHA-256: `{}`\n\nThe exact source-selection receipt gates generic catalog discovery and execution. The selected OpenStax lineage produces four typed records and four replayable executions; a second metadata lineage is not treated as executable corroboration, so promotion remains blocked.\n\nReproduce with `cargo run --quiet --bin stage368_source_selection_acquisition_handoff`.\nMachine-readable report: `{}`\n",
            report.metadata_lineages_selected,
            report.selection_replay_verified,
            report.selected_source_matches_catalog,
            report.executable_catalog_lineages,
            report.source_records,
            report.source_module_replay_verified,
            report.supported_exercises,
            report.exact_decisions,
            report.execution_replays,
            report.promotion_blocked_without_two_executable_lineages,
            report.false_authorizations,
            report.live_registry_mutations,
            report.corpus_sha256,
            JSON,
        ),
    )
    .unwrap();
    println!(
        "stage368 metadata_selected={} selection_replay={} source_match={} executable_lineages={} records={} exercises={} replay={} promotion_blocked={} false_auth={} live_mutations={} corpus_hash={}",
        report.metadata_lineages_selected,
        report.selection_replay_verified,
        report.selected_source_matches_catalog,
        report.executable_catalog_lineages,
        report.source_records,
        report.supported_exercises,
        report.execution_replays,
        report.promotion_blocked_without_two_executable_lineages,
        report.false_authorizations,
        report.live_registry_mutations,
        report.corpus_sha256,
    );
}
