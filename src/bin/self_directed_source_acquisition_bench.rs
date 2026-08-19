//! Integrated Stage D/G shadow source-acquisition campaign.
//!
//! The planner selects a source relation module from exact diagnostic gaps;
//! the selected module is then extracted and validated without being added to
//! the curriculum manifest or production routing.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{
    candidate_is_promotable, cluster_gaps, manifest_unchanged, observe_gap, propose_learning_plans,
    GapKind, SourceModuleCandidate,
};
use the_machine::source_formula_pack::biology_pack::{
    evaluate_biology, BiologyArtifact, BiologyOperation, BiologyRequest,
};
use the_machine::source_formula_pack::source_relation_pack::{
    evaluate_relation, extract_relation_records, RelationRequest,
};

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    observed_cases: usize,
    gap_clusters: usize,
    candidate_plans: usize,
    selected_module: String,
    selected_coverage: usize,
    selected_plan_replay: bool,
    plan_tamper_rejected: bool,
    source_document_sha256: String,
    extracted_records: usize,
    source_mutations: usize,
    source_mutations_rejected: usize,
    independent_validation_cases: usize,
    independent_validation_correct: usize,
    biology_agreements: usize,
    shadow_promotable: bool,
    manifest_unchanged: bool,
    blocked_shortcuts: usize,
    false_authorizations: usize,
    production_authorizations: usize,
    corpus_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("source campaign serializes"))
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let mut observations = Vec::with_capacity(500);
    for index in 0..300 {
        observations.push(observe_gap(
            format!("relation_gap_{index:03}"),
            "dna_complementary_base",
            GapKind::MissingCapability,
            "source relation is absent from the promoted curriculum",
        ));
    }
    for index in 0..120 {
        observations.push(observe_gap(
            format!("biology_gap_{index:03}"),
            "dna_sequence",
            GapKind::MissingCapability,
            "DNA sequence artifact is required downstream",
        ));
    }
    for index in 0..80 {
        observations.push(observe_gap(
            format!("chemistry_gap_{index:03}"),
            "molecular_formula",
            GapKind::MissingCapability,
            "molecular formula artifact is required downstream",
        ));
    }
    assert_eq!(observations.len(), 500);
    let candidates = vec![
        SourceModuleCandidate {
            module_id: "source_relation_dna_complement".into(),
            title: "Extracted complementary-base relation".into(),
            domain: "molecular_biology.dna".into(),
            provides: vec!["dna_complementary_base".into()],
            prerequisite_artifacts: vec!["dna_sequence".into()],
            source_ids: vec!["openstax-biology-2e:dna-complementary-pairing".into()],
            independent_exercise_count: 240,
        },
        SourceModuleCandidate {
            module_id: "source_derived_biology".into(),
            title: "Existing bounded DNA biology pack".into(),
            domain: "molecular_biology".into(),
            provides: vec!["dna_sequence".into()],
            prerequisite_artifacts: vec!["dna_sequence".into()],
            source_ids: vec!["openstax-biology-2e:dna-complementary-pairing".into()],
            independent_exercise_count: 240,
        },
        SourceModuleCandidate {
            module_id: "source_derived_chemistry".into(),
            title: "Existing bounded chemistry pack".into(),
            domain: "chemistry".into(),
            provides: vec!["molecular_formula".into()],
            prerequisite_artifacts: vec!["molecular_formula".into()],
            source_ids: vec!["openstax-chemistry-2e:formulas-stoichiometry".into()],
            independent_exercise_count: 240,
        },
        SourceModuleCandidate {
            module_id: "unproven_relation_shortcut".into(),
            title: "Unproven relation shortcut".into(),
            domain: "molecular_biology.dna".into(),
            provides: vec!["dna_complementary_base".into()],
            prerequisite_artifacts: vec!["dna_sequence".into()],
            source_ids: Vec::new(),
            independent_exercise_count: 0,
        },
    ];
    let plans = propose_learning_plans(&manifest, &observations, &candidates);
    let selected = &plans[0];
    assert_eq!(selected.module_id, "source_relation_dna_complement");
    assert_eq!(selected.covered_case_count, 300);
    let selected_plan_replay = selected.replay_verified();
    let mut tampered_plan = selected.clone();
    tampered_plan.covered_case_count += 1;
    let plan_tamper_rejected = !tampered_plan.replay_verified();

    let document = include_str!("../../docs/sources/openstax_biology_relation_document.txt");
    let records = extract_relation_records(document).expect("selected source extracts");
    let source_document_sha256 = digest(&document);
    let bases = ["A", "T", "C", "G"];
    let mut independent_validation_correct = 0;
    let mut biology_agreements = 0;
    for index in 0..120 {
        let input = bases[index % bases.len()];
        let relation_result = evaluate_relation(
            &RelationRequest {
                relation: "dna_complementary_base".into(),
                input: input.into(),
                domain: "molecular_biology.dna".into(),
                ambiguity: None,
                provenance: vec!["self-directed-source-exercise".into()],
            },
            &records,
        );
        let biology_result = evaluate_biology(&BiologyRequest {
            operation: BiologyOperation::Complement,
            sequence: Some(input.into()),
            orientation: Some("5_to_3".into()),
            domain: "source_derived_bounded_dna".into(),
            ambiguity: None,
            provenance: vec!["self-directed-source-comparison".into()],
        });
        let agrees = relation_result
            .artifact
            .as_ref()
            .and_then(|artifact| match biology_result.artifact.as_ref() {
                Some(BiologyArtifact::PairedComplement { complement, .. })
                    if complement == &artifact.output =>
                {
                    Some(())
                }
                _ => None,
            })
            .is_some();
        if relation_result.authorized() && agrees {
            independent_validation_correct += 1;
        }
        if agrees {
            biology_agreements += 1;
        }
    }
    let mutations = vec![
        document.replace("RELATION_ID: dna_complementary_base", "RELATION_ID: "),
        document.replace("PAIRS: A=T|T=A|C=G|G=C", "PAIRS: A=T|A=G"),
        document.replace("URL: https://", "URL: http://"),
        document.replace(
            "EVIDENCE: A pairs with T and G pairs with C in complementary DNA strands",
            "EVIDENCE: ",
        ),
        document.replace("END RELATION", "BEGIN RELATION"),
        document.replace(
            "ALIASES: DNA complementary base pairing|complementary DNA base",
            "ALIASES: duplicate|duplicate",
        ),
    ];
    let source_mutations_rejected = mutations
        .iter()
        .filter(|mutation| extract_relation_records(mutation).is_err())
        .count();
    let shadow_promotable =
        candidate_is_promotable(selected, 120) && independent_validation_correct == 120;
    let blocked_shortcuts = plans
        .iter()
        .filter(|plan| plan.status == the_machine::curriculum_campaign::PlanStatus::Blocked)
        .count();
    let false_authorizations = plans
        .iter()
        .filter(|plan| plan.status == the_machine::curriculum_campaign::PlanStatus::Proposed)
        .filter(|plan| plan.source_ids.is_empty() || plan.independent_exercise_count == 0)
        .count();
    assert_eq!(records.len(), 1);
    assert_eq!(mutations.len(), 6);
    assert_eq!(source_mutations_rejected, 6);
    assert_eq!(independent_validation_correct, 120);
    assert_eq!(biology_agreements, 120);
    assert_eq!(blocked_shortcuts, 1);
    assert_eq!(false_authorizations, 0);
    assert!(shadow_promotable);
    assert!(selected_plan_replay);
    assert!(plan_tamper_rejected);
    assert!(manifest_unchanged(&manifest_hash, &manifest));
    let report = Report {
        schema: "stage-d-g-self-directed-source-acquisition-v1",
        observed_cases: observations.len(),
        gap_clusters: cluster_gaps(&observations).len(),
        candidate_plans: plans.len(),
        selected_module: selected.module_id.clone(),
        selected_coverage: selected.covered_case_count,
        selected_plan_replay,
        plan_tamper_rejected,
        source_document_sha256,
        extracted_records: records.len(),
        source_mutations: mutations.len(),
        source_mutations_rejected,
        independent_validation_cases: 120,
        independent_validation_correct,
        biology_agreements,
        shadow_promotable,
        manifest_unchanged: manifest_unchanged(&manifest_hash, &manifest),
        blocked_shortcuts,
        false_authorizations,
        production_authorizations: 0,
        corpus_sha256: digest(&(observations, candidates, plans)),
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage-d-g-self-directed-source-acquisition.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
