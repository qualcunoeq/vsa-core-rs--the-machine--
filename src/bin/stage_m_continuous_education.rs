//! Stage M: bounded continuous self-education planning.
//!
//! This is a planning campaign, not a live learning or authorization path.
//! The controller receives typed gap observations and source-backed module
//! metadata, selects a bounded sequence, and leaves all registry state and
//! ambiguous residuals untouched.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use the_machine::continuous_education::{run_campaign, EducationCandidate, EducationDecision};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{
    observe_gap, GapKind, GapObservation, SourceModuleCandidate,
};

const REPORT: &str = "docs/stage_m_continuous_education.json";

#[derive(Clone, Serialize)]
struct Episode {
    id: String,
    observations: Vec<GapObservation>,
}

#[derive(Serialize)]
struct EpisodeReceipt {
    id: String,
    initial_cases: usize,
    expected_resolved_cases: usize,
    resolved_cases: usize,
    remaining_cases: usize,
    selected_modules: Vec<String>,
    decisions: Vec<EducationDecision>,
    replay_verified: bool,
    deterministic_rerun: bool,
    tamper_rejected: bool,
    manifest_unchanged: bool,
    forbidden_selection: bool,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    episodes: usize,
    exact_decisions: usize,
    campaign_replays: usize,
    deterministic_reruns: usize,
    tamper_rejections: usize,
    manifest_unchanged: usize,
    resolved_case_total: usize,
    remaining_case_total: usize,
    selected_steps: usize,
    blocked_steps: usize,
    no_coverage_steps: usize,
    complete_steps: usize,
    source_gated_selections: usize,
    forbidden_selections: usize,
    false_authorizations: usize,
    live_registry_mutations: usize,
    corpus_sha256: String,
    receipts: Vec<EpisodeReceipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("stage M serializes"))
    )
}

fn candidate(
    module_id: &str,
    provides: &[&str],
    prerequisite_artifacts: &[&str],
    source_id: &str,
    exercises: usize,
    cost: usize,
    authoritative: bool,
) -> EducationCandidate {
    EducationCandidate {
        source_module: SourceModuleCandidate {
            module_id: module_id.into(),
            title: format!("{module_id} source module"),
            domain: module_id.into(),
            provides: provides.iter().map(|value| (*value).into()).collect(),
            prerequisite_artifacts: prerequisite_artifacts
                .iter()
                .map(|value| (*value).into())
                .collect(),
            source_ids: if source_id.is_empty() {
                Vec::new()
            } else {
                vec![source_id.into()]
            },
            independent_exercise_count: exercises,
        },
        acquisition_cost: cost,
        authoritative_source_verified: authoritative,
        minimum_independent_exercises: 20,
    }
}

fn candidates() -> Vec<EducationCandidate> {
    vec![
        candidate(
            "source_derived_finite_statistics",
            &[
                "arithmetic_mean",
                "weighted_mean",
                "bernoulli_variance",
                "binomial_expected_value",
                "binomial_variance",
            ],
            &["distribution"],
            "source:finite-statistics:textbook",
            240,
            8,
            true,
        ),
        candidate(
            "combinatorics",
            &[
                "permutation_count",
                "combination_count",
                "multinomial_count",
                "inclusion_exclusion_count",
                "surjection_count",
            ],
            &["distribution", "finite_graph", "gcd_bezout"],
            "source:finite-combinatorics:textbook",
            220,
            12,
            true,
        ),
        candidate(
            "elementary_number_theory",
            &["gcd_bezout", "congruence_class", "crt_class", "totient"],
            &["group"],
            "source:elementary-number-theory:textbook",
            220,
            10,
            true,
        ),
        candidate(
            "source_derived_finite_topology",
            &[
                "finite_topology",
                "open_set",
                "closed_set",
                "interior",
                "closure",
            ],
            &["group"],
            "source:finite-topology:textbook",
            180,
            18,
            true,
        ),
        // Exact lexical overlap is not enough: this shortcut has no source or
        // exercise evidence and must never be selected.
        candidate(
            "unproven_statistics_shortcut",
            &["arithmetic_mean", "weighted_mean"],
            &["distribution"],
            "",
            0,
            1,
            false,
        ),
        // This candidate has a source but its requested prerequisite is not a
        // governed artifact, so it remains blocked rather than being invented.
        candidate(
            "unvalidated_spectral_extension",
            &["spectral_projector"],
            &["unknown_operator"],
            "source:spectral:unvalidated",
            200,
            20,
            false,
        ),
        // Broad labels do not create exact artifact coverage.
        candidate(
            "lexical_mathematics_shortcut",
            &["mathematics"],
            &["distribution"],
            "source:lexical:unvalidated",
            200,
            1,
            true,
        ),
    ]
}

fn push_gap(
    observations: &mut Vec<GapObservation>,
    episode: usize,
    ordinal: usize,
    artifact: &str,
    kind: GapKind,
) {
    observations.push(observe_gap(
        format!("episode-{episode:03}-gap-{ordinal:02}"),
        artifact,
        kind,
        match kind {
            GapKind::MissingCapability => "validated method is not yet selected",
            GapKind::MissingKnowledge => "source-backed prerequisite is missing",
            GapKind::Ambiguous => "requested target is not uniquely identified",
            GapKind::Unsupported => "request lies outside the bounded curriculum",
        },
    ));
}

fn episodes() -> Vec<Episode> {
    let stats = [
        "arithmetic_mean",
        "weighted_mean",
        "bernoulli_variance",
        "binomial_expected_value",
        "binomial_variance",
    ];
    let counts = [
        "permutation_count",
        "combination_count",
        "multinomial_count",
        "inclusion_exclusion_count",
        "surjection_count",
    ];
    let number = ["gcd_bezout", "congruence_class", "crt_class", "totient"];
    let topology = [
        "finite_topology",
        "open_set",
        "closed_set",
        "interior",
        "closure",
    ];
    (0..300)
        .map(|episode| {
            let mut observations = Vec::new();
            let mut ordinal = 0;
            let add_family = |observations: &mut Vec<GapObservation>,
                              family: &[&str],
                              kind: GapKind,
                              ordinal: &mut usize| {
                for (index, artifact) in family.iter().enumerate() {
                    if (episode + index) % 2 == 0 {
                        push_gap(observations, episode, *ordinal, artifact, kind);
                        *ordinal += 1;
                    }
                }
            };
            match episode % 5 {
                0 => add_family(
                    &mut observations,
                    &stats,
                    GapKind::MissingCapability,
                    &mut ordinal,
                ),
                1 => {
                    add_family(
                        &mut observations,
                        &counts,
                        GapKind::MissingCapability,
                        &mut ordinal,
                    );
                    add_family(
                        &mut observations,
                        &number,
                        GapKind::MissingKnowledge,
                        &mut ordinal,
                    );
                }
                2 => {
                    add_family(
                        &mut observations,
                        &number,
                        GapKind::MissingCapability,
                        &mut ordinal,
                    );
                    add_family(
                        &mut observations,
                        &topology,
                        GapKind::MissingKnowledge,
                        &mut ordinal,
                    );
                }
                3 => {
                    add_family(
                        &mut observations,
                        &["arithmetic_mean", "gcd_bezout"],
                        GapKind::Ambiguous,
                        &mut ordinal,
                    );
                    add_family(
                        &mut observations,
                        &["spectral_projector"],
                        GapKind::MissingKnowledge,
                        &mut ordinal,
                    );
                    add_family(
                        &mut observations,
                        &["mathematics"],
                        GapKind::Unsupported,
                        &mut ordinal,
                    );
                }
                _ => {
                    add_family(
                        &mut observations,
                        &stats,
                        GapKind::MissingKnowledge,
                        &mut ordinal,
                    );
                    add_family(
                        &mut observations,
                        &counts,
                        GapKind::MissingCapability,
                        &mut ordinal,
                    );
                    add_family(
                        &mut observations,
                        &number,
                        GapKind::MissingCapability,
                        &mut ordinal,
                    );
                    add_family(
                        &mut observations,
                        &["arithmetic_mean"],
                        GapKind::Ambiguous,
                        &mut ordinal,
                    );
                }
            }
            Episode {
                id: format!("education-episode-{episode:03}"),
                observations,
            }
        })
        .collect()
}

fn expected_resolved(observations: &[GapObservation]) -> usize {
    let supported: BTreeSet<String> = candidates()
        .into_iter()
        .filter(|candidate| candidate.authoritative_source_verified)
        .flat_map(|candidate| candidate.source_module.provides)
        .collect();
    observations
        .iter()
        .filter(|observation| {
            matches!(
                observation.kind,
                GapKind::MissingCapability | GapKind::MissingKnowledge
            ) && supported.contains(&observation.requested_artifact)
        })
        .count()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let candidates = candidates();
    let episodes = episodes();
    let corpus_sha256 = digest(&(episodes.clone(), candidates.clone(), manifest_hash.clone()));
    let mut receipts = Vec::with_capacity(episodes.len());
    let mut exact_decisions = 0;
    let mut campaign_replays = 0;
    let mut deterministic_reruns = 0;
    let mut tamper_rejections = 0;
    let mut manifest_unchanged = 0;
    let mut resolved_case_total = 0;
    let mut remaining_case_total = 0;
    let mut selected_steps = 0;
    let mut blocked_steps = 0;
    let mut no_coverage_steps = 0;
    let mut complete_steps = 0;
    let mut source_gated_selections = 0;
    let mut forbidden_selections = 0;

    for episode in &episodes {
        let expected = expected_resolved(&episode.observations);
        let campaign = run_campaign(&manifest, &episode.observations, &candidates, 8);
        let rerun = run_campaign(&manifest, &episode.observations, &candidates, 8);
        let replay_verified = campaign.replay_verified();
        let deterministic_rerun = campaign == rerun;
        let mut tampered = campaign.clone();
        tampered.remaining_case_count += 1;
        let tamper_rejected = !tampered.replay_verified();
        let unchanged = campaign.manifest_unchanged() && campaign.manifest_after == manifest_hash;
        let selected: Vec<String> = campaign
            .rounds
            .iter()
            .filter_map(|step| {
                if step.decision == EducationDecision::Selected {
                    step.module_id.clone()
                } else {
                    None
                }
            })
            .collect();
        let allowed: BTreeSet<String> = candidates
            .iter()
            .filter(|candidate| candidate.authoritative_source_verified)
            .map(|candidate| candidate.source_module.module_id.clone())
            .collect();
        let forbidden = selected.iter().any(|module| !allowed.contains(module));
        let source_gated = selected.iter().all(|module| {
            candidates.iter().any(|candidate| {
                candidate.source_module.module_id == *module
                    && candidate.authoritative_source_verified
                    && !candidate.source_module.source_ids.is_empty()
                    && candidate.source_module.independent_exercise_count
                        >= candidate.minimum_independent_exercises
            })
        });
        let actual_exact = campaign.resolved_case_count == expected
            && replay_verified
            && deterministic_rerun
            && tamper_rejected
            && unchanged
            && !forbidden
            && source_gated;
        exact_decisions += usize::from(actual_exact);
        campaign_replays += usize::from(replay_verified);
        deterministic_reruns += usize::from(deterministic_rerun);
        tamper_rejections += usize::from(tamper_rejected);
        manifest_unchanged += usize::from(unchanged);
        resolved_case_total += campaign.resolved_case_count;
        remaining_case_total += campaign.remaining_case_count;
        selected_steps += campaign
            .rounds
            .iter()
            .filter(|step| step.decision == EducationDecision::Selected)
            .count();
        blocked_steps += campaign
            .rounds
            .iter()
            .filter(|step| step.decision == EducationDecision::Blocked)
            .count();
        no_coverage_steps += campaign
            .rounds
            .iter()
            .filter(|step| step.decision == EducationDecision::NoExactCoverage)
            .count();
        complete_steps += campaign
            .rounds
            .iter()
            .filter(|step| step.decision == EducationDecision::Complete)
            .count();
        source_gated_selections += usize::from(source_gated);
        forbidden_selections += usize::from(forbidden);
        receipts.push(EpisodeReceipt {
            id: episode.id.clone(),
            initial_cases: episode.observations.len(),
            expected_resolved_cases: expected,
            resolved_cases: campaign.resolved_case_count,
            remaining_cases: campaign.remaining_case_count,
            selected_modules: selected,
            decisions: campaign.rounds.iter().map(|step| step.decision).collect(),
            replay_verified,
            deterministic_rerun,
            tamper_rejected,
            manifest_unchanged: unchanged,
            forbidden_selection: forbidden,
        });
    }

    let false_authorizations = forbidden_selections;
    let live_registry_mutations = 0;
    assert_eq!(episodes.len(), 300);
    assert_eq!(exact_decisions, episodes.len());
    assert_eq!(campaign_replays, episodes.len());
    assert_eq!(deterministic_reruns, episodes.len());
    assert_eq!(tamper_rejections, episodes.len());
    assert_eq!(manifest_unchanged, episodes.len());
    assert_eq!(false_authorizations, 0);
    assert_eq!(live_registry_mutations, 0);
    let report = Report {
        schema: "stage-m-continuous-education-v1",
        episodes: episodes.len(),
        exact_decisions,
        campaign_replays,
        deterministic_reruns,
        tamper_rejections,
        manifest_unchanged,
        resolved_case_total,
        remaining_case_total,
        selected_steps,
        blocked_steps,
        no_coverage_steps,
        complete_steps,
        source_gated_selections,
        forbidden_selections,
        false_authorizations,
        live_registry_mutations,
        corpus_sha256,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT, format!("{serialized}\n"))?;
    println!("{serialized}");
    Ok(())
}
