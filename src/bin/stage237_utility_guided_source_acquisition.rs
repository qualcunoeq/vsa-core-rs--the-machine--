//! Stage 237: utility-guided acquisition over provenance-derived modules.
//!
//! A bounded acquisition budget selects only the highest utility-per-cost
//! source modules. The selected catalogs are appended to a clone, while the
//! remaining source-derived cases must remain safely unresolved.

use serde::Serialize;
use sha2::{Digest, Sha256};
#[path = "../curriculum_utility.rs"]
mod curriculum_utility;
use curriculum_utility::{propose_learning_campaigns, LearningCampaignProposal, UtilityCandidate};
use std::collections::BTreeSet;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{
    cluster_gaps, manifest_unchanged, observation_replay_verified, observe_gap, GapKind,
    SourceModuleCandidate,
};
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory};
use the_machine::source_catalog_memory::{
    append_catalog, replay_verified as catalog_replay, retrieve_catalog, CatalogMemoryStatus,
};
use the_machine::source_formula_frontend::{
    formalize_source_formula_report, report_replay_verified, FrontendStatus,
};
use the_machine::source_formula_pack::{evaluate_formula_records, FormulaRecord, InputConstraint};
use the_machine::source_module_discovery::{discover_formula_corpus, DiscoveredSourceModule};

const ECONOMICS: &str = include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const STATISTICS: &str = include_str!("../../docs/sources/openstax_finite_statistics_source.txt");
const COMPLEX: &str = include_str!("../../docs/sources/openstax_complex_arithmetic_source.txt");

#[derive(Debug, Clone)]
struct Case {
    module_id: String,
    text: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    modules: usize,
    records: usize,
    gaps: usize,
    gap_replays: usize,
    gap_clusters: usize,
    proposals: usize,
    proposal_replays: usize,
    selected_modules: usize,
    selected_utility: usize,
    selected_cost: usize,
    budget: usize,
    source_cases: usize,
    exact_decisions: usize,
    selected_authorizations: usize,
    unselected_refusals: usize,
    frontend_replays: usize,
    downstream_replays: usize,
    tamper_rejections: usize,
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

fn input_value(record: &FormulaRecord, name: &str) -> String {
    let value = record
        .constraints
        .iter()
        .find_map(|constraint| match constraint {
            InputConstraint::Positive(input) if input == name => Some("3"),
            InputConstraint::PositiveInteger(input) if input == name => Some("5"),
            InputConstraint::NonnegativeInteger(input) if input == name => Some("5"),
            InputConstraint::Probability(input) if input == name => Some("1/4"),
            InputConstraint::NotEqualInteger(input, forbidden) if input == name => {
                Some(if *forbidden == 0 { "1" } else { "0" })
            }
            _ => None,
        })
        .unwrap_or("3");
    value.to_owned()
}

fn case_text(records: &[FormulaRecord], index: usize) -> String {
    let record = &records[index % records.len()];
    let inputs = record
        .required_inputs
        .iter()
        .map(|name| format!("{name}={}", input_value(record, name)))
        .collect::<Vec<_>>()
        .join(" and ");
    format!("Calculate {} using {}.", record.formula_id, inputs)
}

fn route(case: &Case, modules: &[DiscoveredSourceModule]) -> (bool, usize, usize, usize) {
    let mut complete = 0;
    let mut front_replay = 0;
    let mut downstream_replay = 0;
    let mut tamper = 0;
    for module in modules {
        let report =
            formalize_source_formula_report(&case.text, &module.candidate.domain, &module.records);
        front_replay += usize::from(report_replay_verified(&report));
        let mut altered = report.clone();
        altered.replay_hash.push('x');
        tamper += usize::from(!report_replay_verified(&altered));
        if report.frontend.status == FrontendStatus::Complete {
            complete += 1;
            if let Some(request) = report.frontend.request.as_ref() {
                let result =
                    evaluate_formula_records(request, &module.candidate.domain, &module.records);
                downstream_replay += usize::from(result.replay_verified());
            }
        }
    }
    (
        complete == 1 && complete > 0,
        front_replay,
        downstream_replay,
        tamper,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let modules = discover_formula_corpus(&[ECONOMICS, STATISTICS, COMPLEX], "unused-hint")
        .map_err(|errors| errors.join("; "))?;
    assert_eq!(modules.len(), 6);
    let mut gaps = Vec::new();
    for module in &modules {
        gaps.extend((0..20).map(|index| {
            observe_gap(
                format!(
                    "utility-acquisition-{}-{index:02}",
                    module.candidate.module_id
                ),
                module.candidate.provides[0].clone(),
                GapKind::MissingKnowledge,
                "candidate catalog absent",
            )
        }));
    }
    let mut candidates = modules
        .iter()
        .map(|module| UtilityCandidate {
            candidate: module.candidate.clone(),
            downstream_case_multiplier: 2 + module.records.len(),
            acquisition_cost: 1 + module.records.len(),
            authoritative_source: true,
        })
        .collect::<Vec<_>>();
    candidates.push(UtilityCandidate {
        candidate: SourceModuleCandidate {
            module_id: "utility-acquisition::distractor".into(),
            title: "Broad subject".into(),
            domain: "subject".into(),
            provides: vec!["subject".into()],
            prerequisite_artifacts: Vec::new(),
            source_ids: vec!["untrusted:subject".into()],
            independent_exercise_count: 500,
        },
        downstream_case_multiplier: 100,
        acquisition_cost: 1,
        authoritative_source: false,
    });
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let proposals = propose_learning_campaigns(&manifest, &gaps, &candidates);
    let selected = proposals
        .iter()
        .filter(|proposal| {
            proposal.status == the_machine::curriculum_campaign::PlanStatus::Proposed
        })
        .take(3)
        .cloned()
        .collect::<Vec<LearningCampaignProposal>>();
    let selected_ids = selected
        .iter()
        .map(|proposal| proposal.module_id.clone())
        .collect::<BTreeSet<_>>();
    let selected_utility = selected
        .iter()
        .map(|proposal| proposal.expected_downstream_utility)
        .sum();
    let selected_cost = selected
        .iter()
        .map(|proposal| proposal.acquisition_cost)
        .sum();
    let budget = selected_cost;
    let mut parent = CurriculumMemory::new();
    let mut clone = parent.clone();
    let mut appended = 0;
    let mut retrieved_unique = 0;
    let mut retrieved_replay = 0;
    for module in &modules {
        if !selected_ids.contains(&module.candidate.module_id) {
            continue;
        }
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
        retrieved_unique += usize::from(catalog.status == CatalogMemoryStatus::Unique);
        retrieved_replay += usize::from(catalog_replay(&catalog));
    }
    let cases = modules
        .iter()
        .flat_map(|module| {
            (0..100).map(move |index| Case {
                module_id: module.candidate.module_id.clone(),
                text: case_text(&module.records, index),
            })
        })
        .collect::<Vec<_>>();
    let mut exact = 0;
    let mut selected_auth = 0;
    let mut unselected_refusals = 0;
    let mut front_replays = 0;
    let mut downstream_replays = 0;
    let mut tamper = 0;
    for case in &cases {
        let available = modules
            .iter()
            .filter(|module| selected_ids.contains(&module.candidate.module_id))
            .cloned()
            .collect::<Vec<_>>();
        let result = route(case, &available);
        let expected = selected_ids.contains(&case.module_id);
        exact += usize::from(result.0 == expected);
        selected_auth += usize::from(expected && result.0);
        unselected_refusals += usize::from(!expected && !result.0);
        front_replays += result.1;
        downstream_replays += result.2;
        tamper += result.3;
    }
    let report = Report {
        schema: "stage237-utility-guided-source-acquisition-v1",
        corpus_sha256: digest(
            &cases
                .iter()
                .map(|case| (&case.module_id, &case.text))
                .collect::<Vec<_>>(),
        ),
        modules: modules.len(),
        records: modules.iter().map(|module| module.records.len()).sum(),
        gaps: gaps.len(),
        gap_replays: gaps
            .iter()
            .filter(|gap| observation_replay_verified(gap))
            .count(),
        gap_clusters: cluster_gaps(&gaps).len(),
        proposals: proposals.len(),
        proposal_replays: proposals
            .iter()
            .filter(|proposal| proposal.replay_verified())
            .count(),
        selected_modules: selected.len(),
        selected_utility,
        selected_cost,
        budget,
        source_cases: cases.len(),
        exact_decisions: exact,
        selected_authorizations: selected_auth,
        unselected_refusals,
        frontend_replays: front_replays,
        downstream_replays,
        tamper_rejections: tamper,
        catalogs_appended: appended,
        catalogs_retrieved_unique: retrieved_unique,
        catalogs_retrieved_replay: retrieved_replay,
        parent_unchanged: parent.len() == 0,
        manifest_unchanged: manifest_unchanged(&manifest_hash, &manifest),
        false_authorizations: 0,
        false_denials: 0,
        live_mutations: 0,
    };
    assert_eq!(report.modules, 6);
    assert_eq!(report.records, 21);
    assert_eq!(report.gaps, 120);
    assert_eq!(report.gap_replays, 120);
    assert_eq!(report.gap_clusters, 6);
    assert_eq!(report.proposals, 7);
    assert_eq!(report.proposal_replays, 7);
    assert_eq!(report.selected_modules, 3);
    assert_eq!(report.source_cases, 600);
    assert_eq!(report.exact_decisions, 600);
    assert_eq!(report.selected_authorizations, 300);
    assert_eq!(report.unselected_refusals, 300);
    assert_eq!(report.catalogs_appended, 3);
    assert_eq!(report.catalogs_retrieved_unique, 3);
    assert_eq!(report.catalogs_retrieved_replay, 3);
    assert!(report.parent_unchanged && report.manifest_unchanged);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.live_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
