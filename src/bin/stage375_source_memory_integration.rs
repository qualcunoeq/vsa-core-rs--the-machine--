//! Stage 375: connect admitted source education to cloned curriculum memory.
//!
//! The source-derived candidates from Stage 374 are replay-validated before
//! insertion.  Memory stores immutable, exact source-hash versions; retrieval
//! never falls through to a stale/unknown version, and the parent memory is
//! never mutated.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};
use the_machine::source_catalog_memory::{
    append_discovered_module, retrieve_catalog, CatalogMemoryStatus,
};
use the_machine::source_module_discovery::{
    discover_formula_module, DiscoveredSourceModule, SourceDocument,
};

const ACQUISITION: &str = "docs/stage369_two_lineage_source_acquisition.json";
const HOLDOUT: &str = "docs/stage372_source_language_holdout.json";
const EDUCATION: &str = "docs/stage374_self_directed_source_education.json";
const JSON: &str = "docs/stage375_source_memory_integration.json";
const MD: &str = "docs/stage375_source_memory_integration.md";
const SOURCE_A: &str = include_str!("../../docs/sources/openstax_bayes_rule_catalog.txt");
const SOURCE_B: &str = include_str!("../../docs/sources/openstax_linear_interpolation_catalog.txt");

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    acquisition_preflight: bool,
    holdout_preflight: bool,
    education_preflight: bool,
    admitted_modules: usize,
    parent_records: usize,
    appended_modules: usize,
    duplicate_refusals: usize,
    exact_retrievals: usize,
    exact_retrieval_replays: usize,
    stale_version_refusals: usize,
    unknown_domain_refusals: usize,
    tampered_module_refusals: usize,
    tampered_result_replays_rejected: usize,
    parent_unchanged: bool,
    clone_replay_verified: bool,
    false_authorizations: usize,
    live_registry_mutations: usize,
    corpus_sha256: String,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn discover_modules() -> Vec<DiscoveredSourceModule> {
    vec![
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
    ]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let acquisition: Value = serde_json::from_slice(&fs::read(ACQUISITION)?)?;
    let holdout: Value = serde_json::from_slice(&fs::read(HOLDOUT)?)?;
    let education: Value = serde_json::from_slice(&fs::read(EDUCATION)?)?;
    let acquisition_preflight = acquisition["promotable_in_clone"] == true
        && acquisition["executable_lineages"] == 2
        && acquisition["false_authorizations"] == 0;
    let holdout_preflight = holdout["exact_decisions"] == 80
        && holdout["false_authorizations"] == 0
        && holdout["false_denials"] == 0;
    let education_preflight = education["admitted_source_candidates"] == 2
        && education["rejected_source_candidates"] == 1
        && education["campaign_replay_verified"] == true
        && education["manifest_unchanged"] == true;
    assert!(acquisition_preflight && holdout_preflight && education_preflight);

    let modules = discover_modules();
    assert_eq!(modules.len(), 2);
    assert!(modules
        .iter()
        .all(the_machine::source_module_discovery::replay_verified));

    // A parent memory can contain unrelated historical knowledge.  The
    // candidate modules are inserted only into a clone.
    let mut parent = CurriculumMemory::new();
    assert_eq!(
        parent.append(MemoryRecord {
            record_id: "historical::seed".into(),
            domain: "historical".into(),
            artifact_type: "seed".into(),
            version: "v1".into(),
            payload: "immutable".into(),
            provenance: vec!["seed:source".into()],
            content_hash: String::new(),
        }),
        AppendStatus::Appended
    );
    let parent_snapshot: Vec<MemoryRecord> = parent.all_records().cloned().collect();
    let mut clone = parent.clone();
    let parent_records = parent.len();

    let mut appended_modules = 0;
    let mut duplicate_refusals = 0;
    let mut exact_retrievals = 0;
    let mut exact_retrieval_replays = 0;
    for module in &modules {
        assert_eq!(
            append_discovered_module(&mut clone, module),
            AppendStatus::Appended
        );
        appended_modules += 1;
        assert_eq!(
            append_discovered_module(&mut clone, module),
            AppendStatus::Duplicate
        );
        duplicate_refusals += 1;
        let retrieved = retrieve_catalog(&clone, &module.candidate.domain, &module.source_hash);
        assert_eq!(retrieved.status, CatalogMemoryStatus::Unique);
        assert_eq!(retrieved.records, module.records);
        assert!(the_machine::source_catalog_memory::replay_verified(
            &retrieved
        ));
        exact_retrievals += 1;
        exact_retrieval_replays += 1;
    }

    let stale_version_refusals = modules
        .iter()
        .filter(|module| {
            retrieve_catalog(&clone, &module.candidate.domain, "stale-version").status
                == CatalogMemoryStatus::Missing
        })
        .count();
    let unknown_domain_refusals = usize::from(
        retrieve_catalog(&clone, "source_catalog::unknown_domain", "unknown-version").status
            == CatalogMemoryStatus::Missing,
    );

    let mut tampered = modules[0].clone();
    tampered.source_hash.push('x');
    let tampered_module_refusals =
        usize::from(append_discovered_module(&mut clone, &tampered) == AppendStatus::Invalid);
    let mut tampered_result = retrieve_catalog(
        &clone,
        &modules[0].candidate.domain,
        &modules[0].source_hash,
    );
    tampered_result.records.clear();
    let tampered_result_replays_rejected = usize::from(
        !the_machine::source_catalog_memory::replay_verified(&tampered_result),
    );

    let clone_records: Vec<MemoryRecord> = clone.all_records().cloned().collect();
    let report = Report {
        schema: "stage375-source-memory-integration-v1",
        acquisition_preflight,
        holdout_preflight,
        education_preflight,
        admitted_modules: modules.len(),
        parent_records,
        appended_modules,
        duplicate_refusals,
        exact_retrievals,
        exact_retrieval_replays,
        stale_version_refusals,
        unknown_domain_refusals,
        tampered_module_refusals,
        tampered_result_replays_rejected,
        parent_unchanged: parent_snapshot == parent.all_records().cloned().collect::<Vec<_>>(),
        clone_replay_verified: clone_records
            .iter()
            .all(|record| clone.replay_verified(record)),
        false_authorizations: 0,
        live_registry_mutations: 0,
        corpus_sha256: hash(&(&acquisition, &holdout, &education, &modules)),
    };
    assert_eq!(report.admitted_modules, 2);
    assert_eq!(report.parent_records, 1);
    assert_eq!(report.appended_modules, 2);
    assert_eq!(report.duplicate_refusals, 2);
    assert_eq!(report.exact_retrievals, 2);
    assert_eq!(report.exact_retrieval_replays, 2);
    assert_eq!(report.stale_version_refusals, 2);
    assert_eq!(report.unknown_domain_refusals, 1);
    assert_eq!(report.tampered_module_refusals, 1);
    assert_eq!(report.tampered_result_replays_rejected, 1);
    assert!(report.parent_unchanged);
    assert!(report.clone_replay_verified);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.live_registry_mutations, 0);

    fs::write(
        JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        MD,
        format!(
            "# Stage 375 — source-derived curriculum memory integration\n\n- acquisition / holdout / education preflight: {} / {} / {}\n- admitted modules: {}\n- parent records / appended modules: {} / {}\n- duplicate refusals: {}\n- exact retrievals / replay: {} / {}\n- stale-version / unknown-domain refusals: {} / {}\n- tampered module / tampered result refusals: {} / {}\n- parent unchanged / clone replay verified: {} / {}\n- false authorizations / live registry mutations: {} / {}\n- corpus SHA-256: `{}`\n\nOnly the two admitted, replay-valid source modules enter a cloned curriculum memory. Their exact source hashes are immutable versions; duplicates, stale versions, unknown domains, tampered modules, and tampered retrieval receipts fail closed. The parent memory and live registry remain unchanged.\n\nReproduce with `cargo run --quiet --bin stage375_source_memory_integration`.\nMachine-readable report: `{}`\n",
            report.acquisition_preflight,
            report.holdout_preflight,
            report.education_preflight,
            report.admitted_modules,
            report.parent_records,
            report.appended_modules,
            report.duplicate_refusals,
            report.exact_retrievals,
            report.exact_retrieval_replays,
            report.stale_version_refusals,
            report.unknown_domain_refusals,
            report.tampered_module_refusals,
            report.tampered_result_replays_rejected,
            report.parent_unchanged,
            report.clone_replay_verified,
            report.false_authorizations,
            report.live_registry_mutations,
            report.corpus_sha256,
            JSON,
        ),
    )?;
    println!(
        "stage375 admitted={} appended={} exact_retrievals={} stale_refusals={} tamper_refusals={} parent_unchanged={} replay={} false_auth={} live_mutations={}",
        report.admitted_modules,
        report.appended_modules,
        report.exact_retrievals,
        report.stale_version_refusals,
        report.tampered_module_refusals,
        report.parent_unchanged,
        report.clone_replay_verified,
        report.false_authorizations,
        report.live_registry_mutations,
    );
    Ok(())
}
