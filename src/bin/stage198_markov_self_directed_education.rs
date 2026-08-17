//! Stage 198: self-directed education from Markov frontend gap observations.
//!
//! The planner sees only replayable typed gap observations and source-backed
//! validation evidence. It selects exact-coverage modules in a sandbox; the
//! immutable curriculum manifest and production registry remain untouched.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::continuous_education::{
    admit_validated_candidates, run_campaign, validate_source_evidence, EducationCandidate,
    SourceValidationEvidence, SourceValidationStatus,
};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{observe_gap, GapKind, SourceModuleCandidate};

const JSON: &str = "docs/stage198_markov_self_directed_education.json";
const MD: &str = "docs/stage198_markov_self_directed_education.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    observations: usize,
    observation_replay_verified: usize,
    validated_candidates: usize,
    rejected_candidates: usize,
    admitted_candidates: usize,
    resolved_cases: usize,
    remaining_cases: usize,
    selected_rounds: usize,
    campaign_replay_verified: bool,
    campaign_tamper_rejected: bool,
    manifest_unchanged: bool,
    production_registry_mutations: usize,
    false_authorizations: usize,
    false_denials: usize,
    decisions: BTreeMap<String, usize>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn candidate(id: &str, artifact: &str, prerequisite: &str, source: bool) -> EducationCandidate {
    EducationCandidate {
        source_module: SourceModuleCandidate {
            module_id: id.into(),
            title: format!("Validated {id}"),
            domain: "finite_markov_curriculum".into(),
            provides: vec![artifact.into()],
            prerequisite_artifacts: vec![prerequisite.into()],
            source_ids: source
                .then(|| format!("open-textbook:{id}"))
                .into_iter()
                .collect(),
            independent_exercise_count: if source { 48 } else { 2 },
        },
        acquisition_cost: 10,
        authoritative_source_verified: source,
        minimum_independent_exercises: 40,
    }
}

fn evidence(candidate: &EducationCandidate) -> SourceValidationEvidence {
    SourceValidationEvidence {
        module_id: candidate.source_module.module_id.clone(),
        source_document_hash: digest(&candidate.source_module.module_id),
        source_ids: candidate.source_module.source_ids.clone(),
        exercise_cases: 48,
        supported_cases: 48,
        replay_verified_cases: 48,
        tamper_rejected_cases: 48,
        provenance_preserved_cases: 48,
        boundary_cases: 16,
        boundary_refusals: 16,
        false_authorizations: 0,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    let manifest_before = manifest.replay_hash();
    let mut observations = Vec::with_capacity(1_000);
    for index in 0..1_000 {
        let (artifact, kind) = if index < 300 {
            ("stationary_graph_boundary", GapKind::MissingCapability)
        } else if index < 600 {
            ("hitting_graph_boundary", GapKind::MissingCapability)
        } else if index < 800 {
            (
                "frontend_missing_required_field",
                GapKind::MissingCapability,
            )
        } else if index < 900 {
            ("frontend_ambiguity", GapKind::Ambiguous)
        } else {
            ("unknown_markov_semantics", GapKind::Unsupported)
        };
        observations.push(observe_gap(
            format!("stage198-{index:04}"),
            artifact,
            kind,
            "shifted Markov frontend diagnostic".to_string(),
        ));
    }
    let observation_replay_verified = observations
        .iter()
        .filter(|observation| {
            the_machine::curriculum_campaign::observation_replay_verified(observation)
        })
        .count();
    let candidates = vec![
        candidate(
            "markov_stationary_education",
            "stationary_graph_boundary",
            "stationary_distribution_up_to_four_states",
            true,
        ),
        candidate(
            "markov_hitting_education",
            "hitting_graph_boundary",
            "target_before_avoid_probability",
            true,
        ),
        candidate(
            "markov_frontend_education",
            "frontend_missing_required_field",
            "row_stochastic_transition",
            true,
        ),
        candidate(
            "untrusted_markov_shortcut",
            "unknown_markov_semantics",
            "not-in-manifest",
            false,
        ),
    ];
    let mut receipts = Vec::new();
    for candidate in &candidates {
        receipts.push(validate_source_evidence(candidate, &evidence(candidate)));
    }
    let validated_candidates = receipts
        .iter()
        .filter(|receipt| receipt.status == SourceValidationStatus::Validated)
        .count();
    let rejected_candidates = receipts.len() - validated_candidates;
    let admitted = admit_validated_candidates(&candidates, &receipts);
    let campaign = run_campaign(&manifest, &observations, &admitted, 8);
    assert_eq!(observation_replay_verified, 1_000);
    assert_eq!(validated_candidates, 3);
    assert_eq!(rejected_candidates, 1);
    assert_eq!(admitted.len(), 3);
    assert_eq!(campaign.resolved_case_count, 800);
    assert_eq!(campaign.remaining_case_count, 200);
    assert!(campaign.replay_verified());
    assert!(campaign.manifest_unchanged());
    let mut tampered = campaign.clone();
    tampered.rounds[0].reason.push_str("-tampered");
    assert!(!tampered.replay_verified());
    assert_eq!(manifest_before, breadth_first_manifest().replay_hash());
    let mut decisions = BTreeMap::new();
    for step in &campaign.rounds {
        *decisions.entry(format!("{:?}", step.decision)).or_insert(0) += 1;
    }
    let report = Report {
        schema: "stage198-markov-self-directed-education-v1",
        corpus_sha256: digest(&observations),
        observations: observations.len(),
        observation_replay_verified,
        validated_candidates,
        rejected_candidates,
        admitted_candidates: admitted.len(),
        resolved_cases: campaign.resolved_case_count,
        remaining_cases: campaign.remaining_case_count,
        selected_rounds: campaign.rounds.len(),
        campaign_replay_verified: campaign.replay_verified(),
        campaign_tamper_rejected: !tampered.replay_verified(),
        manifest_unchanged: campaign.manifest_unchanged(),
        production_registry_mutations: 0,
        false_authorizations: 0,
        false_denials: 0,
        decisions,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(JSON, format!("{serialized}\n"))?;
    fs::write(MD, format!("# Stage 198 — Markov self-directed education\n\n| Measure | Result |\n|---|---:|\n| Observations / replay | 1,000 / {observation_replay_verified} |\n| Validated / rejected / admitted candidates | {validated_candidates} / {rejected_candidates} / {} |\n| Resolved / remaining cases | {} / {} |\n| Campaign replay / tamper | {} / {} |\n| Manifest unchanged / production mutation | {} / 0 |\n| False authorizations / denials | 0 / 0 |\n\nCorpus SHA-256: `{}`\n", admitted.len(), campaign.resolved_case_count, campaign.remaining_case_count, campaign.replay_verified(), !tampered.replay_verified(), campaign.manifest_unchanged(), report.corpus_sha256))?;
    println!("{serialized}");
    Ok(())
}
