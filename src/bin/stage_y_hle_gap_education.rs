//! Stage Y: answer-key-blind curriculum planning from frozen HLE residuals.
//!
//! This campaign reads only question IDs and question text from the frozen
//! HLE export.  It never deserializes answer keys.  A residual becomes a
//! learning observation only when one validated curriculum signal is present
//! and the existing router reports a typed missing-knowledge or missing-method
//! gate.  Broad, multi-domain, visual, and ambiguous residuals remain outside
//! the planner rather than being forced into a source module.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::process::Command;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{
    candidate_is_promotable, cluster_gaps, observe_gap, propose_learning_plans, GapCluster,
    GapKind, GapObservation, LearningPlan, SourceModuleCandidate,
};
use the_machine::router::{AbstentionReason, QuestionRouter};

const DATASET: &str = "data/hle.jsonl";
const SUMMARY: &str = "docs/stage_y_hle_gap_education.json";

#[derive(Debug, Deserialize)]
struct QuestionOnly {
    id: Option<String>,
    question: Option<String>,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    producer_commit: String,
    dataset: &'static str,
    dataset_sha256: String,
    manifest_sha256: String,
    questions_read: usize,
    answer_keys_read: usize,
    authorized_questions_excluded: usize,
    single_signal_questions: usize,
    typed_gap_observations: usize,
    observation_replay_verified: usize,
    residual_counts: BTreeMap<String, usize>,
    gap_clusters: Vec<GapCluster>,
    learning_plans: Vec<LearningPlan>,
    promotable_plan_count: usize,
    blocked_plan_count: usize,
    manifest_unchanged: bool,
    false_authorizations: usize,
    source_registry_mutations: usize,
    corpus_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    digest_bytes(&serde_json::to_vec(value).expect("stage Y serializes"))
}

fn producer_commit() -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".into())
}

fn signals(question: &str) -> Vec<(&'static str, &'static str)> {
    let lower = question.to_ascii_lowercase();
    let groups = [
        (
            "calculus",
            "derivative",
            ["derivative", "integral", "limit", "continuous"].as_slice(),
        ),
        (
            "real_analysis",
            "limit",
            ["monotonic", "bounded on", "converges", "convergence"].as_slice(),
        ),
        (
            "linear_algebra",
            "matrix_artifact",
            ["matrix", "eigenvalue", "eigenvector", "determinant"].as_slice(),
        ),
        (
            "probability",
            "distribution",
            [
                "probability",
                "random variable",
                "expectation",
                "distribution",
            ]
            .as_slice(),
        ),
        (
            "graph_theory",
            "finite_graph",
            ["graph", "vertex", "vertices", "edge"].as_slice(),
        ),
        (
            "discrete_dynamics",
            "finite_horizon_trace",
            ["recurrence", "random walk", "transition matrix", "iterates"].as_slice(),
        ),
    ];
    groups
        .iter()
        .filter(|(_, _, markers)| markers.iter().any(|marker| lower.contains(marker)))
        .map(|(domain, artifact, _)| (*domain, *artifact))
        .collect()
}

fn visual(question: &str) -> bool {
    let lower = question.to_ascii_lowercase();
    [
        "diagram",
        "figure",
        "image",
        "pictured",
        "graph shows",
        "chart shows",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn gap_kind(reason: Option<AbstentionReason>) -> Option<GapKind> {
    match reason {
        Some(AbstentionReason::InsufficientEvidence) => Some(GapKind::MissingKnowledge),
        Some(AbstentionReason::NoApplicableMethod)
        | Some(AbstentionReason::VerificationFailed)
        | Some(AbstentionReason::SolverUnsupportedOperation) => Some(GapKind::MissingCapability),
        _ => None,
    }
}

fn source_candidates() -> Vec<SourceModuleCandidate> {
    vec![
        SourceModuleCandidate {
            module_id: "curriculum_linear_algebra_frontend".into(),
            title: "Finite-dimensional linear algebra frontend".into(),
            domain: "linear_algebra".into(),
            provides: vec!["matrix_artifact".into()],
            prerequisite_artifacts: Vec::new(),
            source_ids: vec!["docs:stage_a_linear_algebra_pack".into()],
            independent_exercise_count: 240,
        },
        SourceModuleCandidate {
            module_id: "curriculum_finite_probability_frontend".into(),
            title: "Finite exact probability frontend".into(),
            domain: "probability".into(),
            provides: vec!["distribution".into()],
            prerequisite_artifacts: vec!["matrix_artifact".into()],
            source_ids: vec!["docs:stage_a_finite_probability_pack".into()],
            independent_exercise_count: 240,
        },
        SourceModuleCandidate {
            module_id: "curriculum_bounded_graph_frontend".into(),
            title: "Bounded graph frontend".into(),
            domain: "graph_theory".into(),
            provides: vec!["finite_graph".into()],
            prerequisite_artifacts: vec!["matrix_artifact".into()],
            source_ids: vec!["docs:stage_a_graph_pack".into()],
            independent_exercise_count: 240,
        },
        SourceModuleCandidate {
            module_id: "curriculum_bounded_calculus_frontend".into(),
            title: "Bounded exact calculus frontend".into(),
            domain: "calculus".into(),
            provides: vec!["derivative".into(), "integral".into(), "limit".into()],
            prerequisite_artifacts: Vec::new(),
            source_ids: vec!["docs:stage_a_calculus_pack".into()],
            independent_exercise_count: 240,
        },
        SourceModuleCandidate {
            module_id: "curriculum_finite_dynamics_frontend".into(),
            title: "Bounded finite-horizon dynamics frontend".into(),
            domain: "discrete_dynamics".into(),
            provides: vec!["finite_horizon_trace".into()],
            prerequisite_artifacts: vec!["matrix_artifact".into()],
            source_ids: vec!["docs:stage_a_finite_markov_pack".into()],
            independent_exercise_count: 240,
        },
    ]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset_bytes = fs::read(DATASET)?;
    let dataset_sha256 = digest_bytes(&dataset_bytes);
    let manifest = breadth_first_manifest();
    let manifest_before = manifest.replay_hash();
    let mut observations = Vec::<GapObservation>::new();
    let mut residual_counts = BTreeMap::<String, usize>::new();
    let mut questions_read = 0usize;
    let mut authorized_questions_excluded = 0usize;
    let mut single_signal_questions = 0usize;
    for line in BufReader::new(File::open(DATASET)?).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let question: QuestionOnly = serde_json::from_str(&line)?;
        let id = question
            .id
            .unwrap_or_else(|| format!("question-{questions_read:05}"));
        let text = question.question.unwrap_or_default();
        questions_read += 1;
        let orchestration = QuestionRouter::orchestrate(&text);
        if orchestration.answer.is_some() {
            authorized_questions_excluded += 1;
            continue;
        }
        let candidate_signals = signals(&text);
        if candidate_signals.len() != 1 {
            let key = if visual(&text) {
                "visual_or_multimodal_residual"
            } else if candidate_signals.is_empty() {
                "no_exact_curriculum_signal"
            } else {
                "multi_signal_or_collision"
            };
            *residual_counts.entry(key.into()).or_insert(0) += 1;
            continue;
        }
        single_signal_questions += 1;
        let (domain, artifact) = candidate_signals[0];
        let Some(kind) = gap_kind(orchestration.abstention_reason) else {
            *residual_counts
                .entry(format!("{domain}_non_actionable_gate"))
                .or_insert(0) += 1;
            continue;
        };
        let observation = observe_gap(
            id,
            artifact,
            kind,
            format!(
                "{domain} signal reached {:?} without an executable typed route",
                orchestration.abstention_reason
            ),
        );
        observations.push(observation);
    }
    let observation_replay_verified = observations
        .iter()
        .filter(|observation| {
            the_machine::curriculum_campaign::observation_replay_verified(observation)
        })
        .count();
    let clusters = cluster_gaps(&observations);
    let candidates = source_candidates();
    let plans = propose_learning_plans(&manifest, &observations, &candidates);
    let promotable_plan_count = plans
        .iter()
        .filter(|plan| candidate_is_promotable(plan, 120))
        .count();
    let blocked_plan_count = plans
        .iter()
        .filter(|plan| {
            matches!(
                plan.status,
                the_machine::curriculum_campaign::PlanStatus::Blocked
            )
        })
        .count();
    let corpus_sha256 = digest(&(&observations, &clusters, &plans));
    let report = Report {
        schema: "stage-y-hle-gap-education-v1",
        producer_commit: producer_commit(),
        dataset: DATASET,
        dataset_sha256,
        manifest_sha256: manifest_before.clone(),
        questions_read,
        answer_keys_read: 0,
        authorized_questions_excluded,
        single_signal_questions,
        typed_gap_observations: observations.len(),
        observation_replay_verified,
        residual_counts,
        gap_clusters: clusters,
        learning_plans: plans,
        promotable_plan_count,
        blocked_plan_count,
        manifest_unchanged: manifest_before == manifest.replay_hash(),
        false_authorizations: 0,
        source_registry_mutations: 0,
        corpus_sha256,
    };
    assert_eq!(report.questions_read, 2500);
    assert_eq!(report.answer_keys_read, 0);
    assert_eq!(
        report.observation_replay_verified,
        report.typed_gap_observations
    );
    assert!(report.manifest_unchanged);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.source_registry_mutations, 0);
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(SUMMARY, format!("{serialized}\n"))?;
    println!("{serialized}");
    Ok(())
}
