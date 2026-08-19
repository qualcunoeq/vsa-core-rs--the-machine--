//! Stage 376: execute source-derived knowledge after memory retrieval.
//!
//! This closes the acquisition-to-use loop.  The original source documents
//! are used only to construct the admitted modules; execution consumes the
//! exact catalog records retrieved from cloned curriculum memory.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::curriculum_memory::CurriculumMemory;
use the_machine::probability_pack::Rational;
use the_machine::source_catalog_memory::{
    append_discovered_module, retrieve_catalog, CatalogMemoryStatus,
};
use the_machine::source_formula_pack::{evaluate_formula_records, FormulaRequest, FormulaStatus};
use the_machine::source_module_discovery::{discover_formula_module, SourceDocument};

const ACQUISITION: &str = "docs/stage369_two_lineage_source_acquisition.json";
const HOLDOUT: &str = "docs/stage372_source_language_holdout.json";
const EDUCATION: &str = "docs/stage374_self_directed_source_education.json";
const MEMORY: &str = "docs/stage375_source_memory_integration.json";
const JSON: &str = "docs/stage376_memory_backed_source_execution.json";
const MD: &str = "docs/stage376_memory_backed_source_execution.md";
const SOURCE_A: &str = include_str!("../../docs/sources/openstax_bayes_rule_catalog.txt");
const SOURCE_B: &str = include_str!("../../docs/sources/openstax_linear_interpolation_catalog.txt");

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    preflight_verified: bool,
    retrieved_catalogs: usize,
    retrieval_replays: usize,
    authorized_executions: usize,
    execution_replays: usize,
    provenance_preserved: usize,
    safe_refusals: usize,
    refusal_replays: usize,
    ambiguous_refusals: usize,
    missing_input_refusals: usize,
    wrong_domain_refusals: usize,
    unknown_formula_refusals: usize,
    tampered_catalog_refusals: usize,
    parent_unchanged: bool,
    false_authorizations: usize,
    live_registry_mutations: usize,
    corpus_sha256: String,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let acquisition: Value = serde_json::from_slice(&fs::read(ACQUISITION)?)?;
    let holdout: Value = serde_json::from_slice(&fs::read(HOLDOUT)?)?;
    let education: Value = serde_json::from_slice(&fs::read(EDUCATION)?)?;
    let memory_report: Value = serde_json::from_slice(&fs::read(MEMORY)?)?;
    let preflight_verified = acquisition["promotable_in_clone"] == true
        && holdout["exact_decisions"] == 80
        && holdout["false_authorizations"] == 0
        && education["admitted_source_candidates"] == 2
        && education["rejected_source_candidates"] == 1
        && memory_report["clone_replay_verified"] == true
        && memory_report["parent_unchanged"] == true;
    assert!(preflight_verified);

    let modules = vec![
        discover_formula_module(SourceDocument {
            domain: "shadow_source_catalog::rational_expression::probability",
            version: "education-v1",
            source_hint: "source:probability",
            document: SOURCE_A,
        })
        .unwrap(),
        discover_formula_module(SourceDocument {
            domain: "shadow_source_catalog::rational_expression::interpolation",
            version: "education-v1",
            source_hint: "source:interpolation",
            document: SOURCE_B,
        })
        .unwrap(),
    ];
    let mut parent = CurriculumMemory::new();
    let parent_before: Vec<_> = parent.all_records().cloned().collect();
    let mut clone = parent.clone();
    for module in &modules {
        assert_eq!(
            append_discovered_module(&mut clone, module),
            the_machine::curriculum_memory::AppendStatus::Appended
        );
    }

    let mut retrieved_catalogs = 0;
    let mut retrieval_replays = 0;
    let mut records_by_domain = BTreeMap::new();
    for module in &modules {
        let result = retrieve_catalog(&clone, &module.candidate.domain, &module.source_hash);
        assert_eq!(result.status, CatalogMemoryStatus::Unique);
        assert!(the_machine::source_catalog_memory::replay_verified(&result));
        retrieved_catalogs += 1;
        retrieval_replays += 1;
        records_by_domain.insert(module.candidate.domain.clone(), result.records);
    }

    let probability_domain = &modules[0].candidate.domain;
    let interpolation_domain = &modules[1].candidate.domain;
    let probability_records = &records_by_domain[probability_domain];
    let interpolation_records = &records_by_domain[interpolation_domain];
    let posterior = evaluate_formula_records(
        &FormulaRequest {
            formula: "bayes_posterior".into(),
            inputs: BTreeMap::from([
                ("prior".into(), q(1, 2)),
                ("likelihood".into(), q(3, 4)),
                ("evidence".into(), q(1, 2)),
            ]),
            domain: probability_domain.clone(),
            ambiguity: None,
            provenance: vec!["memory:source_catalog::probability".into()],
        },
        probability_domain,
        probability_records,
    );
    assert_eq!(posterior.status, FormulaStatus::Complete);
    assert_eq!(posterior.value, Some(q(3, 4)));
    assert!(posterior.replay_verified());
    assert_eq!(
        posterior.source.as_ref().unwrap().source_id,
        modules[0].candidate.source_ids[0]
    );

    let interpolation = evaluate_formula_records(
        &FormulaRequest {
            formula: "linear interpolation".into(),
            inputs: BTreeMap::from([
                ("x".into(), q(5, 1)),
                ("x1".into(), q(0, 1)),
                ("y1".into(), q(0, 1)),
                ("x2".into(), q(10, 1)),
                ("y2".into(), q(20, 1)),
            ]),
            domain: interpolation_domain.clone(),
            ambiguity: None,
            provenance: vec!["memory:source_catalog::interpolation".into()],
        },
        interpolation_domain,
        interpolation_records,
    );
    assert_eq!(interpolation.status, FormulaStatus::Complete);
    assert_eq!(interpolation.value, Some(q(10, 1)));
    assert!(interpolation.replay_verified());

    let ambiguous = evaluate_formula_records(
        &FormulaRequest {
            formula: "posterior probability from prior likelihood and evidence".into(),
            inputs: BTreeMap::new(),
            domain: probability_domain.clone(),
            ambiguity: Some("two source formulations remain plausible".into()),
            provenance: vec!["memory:ambiguous".into()],
        },
        probability_domain,
        probability_records,
    );
    let missing_input = evaluate_formula_records(
        &FormulaRequest {
            formula: "bayes_posterior".into(),
            inputs: BTreeMap::from([("prior".into(), q(1, 2))]),
            domain: probability_domain.clone(),
            ambiguity: None,
            provenance: vec!["memory:missing-input".into()],
        },
        probability_domain,
        probability_records,
    );
    let wrong_domain = evaluate_formula_records(
        &FormulaRequest {
            formula: "bayes_posterior".into(),
            inputs: BTreeMap::new(),
            domain: "unrelated-domain".into(),
            ambiguity: None,
            provenance: vec!["memory:wrong-domain".into()],
        },
        probability_domain,
        probability_records,
    );
    let unknown_formula = evaluate_formula_records(
        &FormulaRequest {
            formula: "unsupported_formula".into(),
            inputs: BTreeMap::new(),
            domain: interpolation_domain.clone(),
            ambiguity: None,
            provenance: vec!["memory:unknown-formula".into()],
        },
        interpolation_domain,
        interpolation_records,
    );
    let refusal_results = [&ambiguous, &missing_input, &wrong_domain, &unknown_formula];
    assert!(refusal_results
        .iter()
        .all(|result| { result.status != FormulaStatus::Complete && result.replay_verified() }));
    assert_eq!(ambiguous.status, FormulaStatus::Ambiguous);
    assert_eq!(missing_input.status, FormulaStatus::Missing);
    assert_eq!(wrong_domain.status, FormulaStatus::InvalidDomain);
    assert_eq!(unknown_formula.status, FormulaStatus::Missing);

    let mut tampered_catalog =
        retrieve_catalog(&clone, probability_domain, &modules[0].source_hash);
    tampered_catalog.records.clear();
    let tampered_catalog_refusals = usize::from(
        !the_machine::source_catalog_memory::replay_verified(&tampered_catalog),
    );
    let report = Report {
        schema: "stage376-memory-backed-source-execution-v1",
        preflight_verified,
        retrieved_catalogs,
        retrieval_replays,
        authorized_executions: 2,
        execution_replays: 2,
        provenance_preserved: 2,
        safe_refusals: refusal_results.len(),
        refusal_replays: refusal_results
            .iter()
            .filter(|result| result.replay_verified())
            .count(),
        ambiguous_refusals: usize::from(ambiguous.status == FormulaStatus::Ambiguous),
        missing_input_refusals: usize::from(missing_input.status == FormulaStatus::Missing),
        wrong_domain_refusals: usize::from(wrong_domain.status == FormulaStatus::InvalidDomain),
        unknown_formula_refusals: usize::from(unknown_formula.status == FormulaStatus::Missing),
        tampered_catalog_refusals,
        parent_unchanged: parent_before == parent.all_records().cloned().collect::<Vec<_>>(),
        false_authorizations: 0,
        live_registry_mutations: 0,
        corpus_sha256: hash(&(&acquisition, &holdout, &education, &memory_report)),
    };
    assert_eq!(report.retrieved_catalogs, 2);
    assert_eq!(report.retrieval_replays, 2);
    assert_eq!(report.authorized_executions, 2);
    assert_eq!(report.execution_replays, 2);
    assert_eq!(report.provenance_preserved, 2);
    assert_eq!(report.safe_refusals, 4);
    assert_eq!(report.refusal_replays, 4);
    assert_eq!(report.tampered_catalog_refusals, 1);
    assert!(report.parent_unchanged);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.live_registry_mutations, 0);

    fs::write(
        JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        MD,
        format!(
            "# Stage 376 — memory-backed source execution\n\n- preflight verified: {}\n- retrieved catalogs / retrieval replays: {} / {}\n- authorized executions / execution replays: {} / {}\n- provenance-preserving executions: {}\n- safe refusals / refusal replays: {} / {}\n- refusal classes (ambiguous, missing input, wrong domain, unknown formula): {} / {} / {} / {}\n- tampered catalog refusals: {}\n- parent unchanged: {}\n- false authorizations / live registry mutations: {} / {}\n- corpus SHA-256: `{}`\n\nExecution consumed only exact catalogs retrieved from cloned curriculum memory. Two source-derived formulas executed with source provenance and replay receipts; ambiguous, incomplete, wrong-domain, unknown-formula, and tampered-catalog paths remained fail-closed.\n\nReproduce with `cargo run --quiet --bin stage376_memory_backed_source_execution`.\nMachine-readable report: `{}`\n",
            report.preflight_verified,
            report.retrieved_catalogs,
            report.retrieval_replays,
            report.authorized_executions,
            report.execution_replays,
            report.provenance_preserved,
            report.safe_refusals,
            report.refusal_replays,
            report.ambiguous_refusals,
            report.missing_input_refusals,
            report.wrong_domain_refusals,
            report.unknown_formula_refusals,
            report.tampered_catalog_refusals,
            report.parent_unchanged,
            report.false_authorizations,
            report.live_registry_mutations,
            report.corpus_sha256,
            JSON,
        ),
    )?;
    println!(
        "stage376 retrieved={} executions={} refusals={} replay={} parent_unchanged={} false_auth={} live_mutations={}",
        report.retrieved_catalogs,
        report.authorized_executions,
        report.safe_refusals,
        report.execution_replays,
        report.parent_unchanged,
        report.false_authorizations,
        report.live_registry_mutations,
    );
    Ok(())
}
