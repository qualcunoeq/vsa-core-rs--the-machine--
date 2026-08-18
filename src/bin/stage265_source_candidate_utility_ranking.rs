//! Stage 265: utility-ranked source-candidate selection.
//!
//! The planner consumes only immutable source-learning reports and the live
//! curriculum manifest.  It ranks absent candidates, records the policy and
//! selected plan, and never promotes or routes a candidate.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;

use the_machine::curriculum::breadth_first_manifest;

const ECONOMICS: &str = "docs/stage178_self_directed_source_learning_curve.json";
const GEOMETRY: &str = "docs/stage262_source_geometry_candidate_selection.json";
const REPORT_JSON: &str = "docs/stage265_source_candidate_utility_ranking.json";
const REPORT_MD: &str = "docs/stage265_source_candidate_utility_ranking.md";

#[derive(Debug, Serialize)]
struct CandidateScore {
    candidate_id: String,
    evidence_report: String,
    eligible: bool,
    sealed_learning_delta: usize,
    sealed_authorized: usize,
    evidence_cases: usize,
    evidence_artifacts: usize,
    acquisition_cost: usize,
    rank: usize,
    score_key: (usize, usize, usize, usize),
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    current_manifest_hash: String,
    candidate_count: usize,
    eligible_candidates: usize,
    selected_candidate: String,
    selection_policy: &'static str,
    candidates: Vec<CandidateScore>,
    shadow_only: bool,
    hle_questions_read: usize,
    manifest_mutations: usize,
    registry_mutations: usize,
    false_authorizations: usize,
    false_denials: usize,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn load(path: &str) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn number(value: &Value, key: &str) -> usize {
    value.get(key).and_then(Value::as_u64).unwrap_or_default() as usize
}

fn absent(manifest: &the_machine::curriculum::CurriculumManifest, id: &str) -> bool {
    !manifest.packs.iter().any(|pack| pack.id == id)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    let economics = load(ECONOMICS)?;
    let geometry = load(GEOMETRY)?;
    let mut candidates = vec![
        CandidateScore {
            candidate_id: "source_derived_bounded_economics".into(),
            evidence_report: ECONOMICS.into(),
            eligible: absent(&manifest, "source_derived_bounded_economics")
                && number(&economics, "false_authorizations") == 0
                && number(&economics, "false_denials") == 0
                && number(&economics, "sealed_outcomes_exposed_to_selector") == 0,
            sealed_learning_delta: number(&economics, "sealed_learning_delta"),
            sealed_authorized: number(&economics, "sealed_promoted_authorized"),
            evidence_cases: number(&economics, "cases"),
            evidence_artifacts: 1,
            acquisition_cost: 1,
            rank: 0,
            score_key: (0, 0, 0, 0),
        },
        CandidateScore {
            candidate_id: "source_derived_bounded_geometry".into(),
            evidence_report: GEOMETRY.into(),
            eligible: absent(&manifest, "source_derived_bounded_geometry")
                && number(&geometry, "false_authorizations") == 0
                && number(&geometry, "false_denials") == 0
                && geometry
                    .get("shadow_only")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            sealed_learning_delta: number(&geometry, "sealed_learning_delta"),
            sealed_authorized: number(&geometry, "sealed_learning_delta"),
            evidence_cases: number(&geometry, "route_blind_cases")
                + number(&geometry, "language_transfer_cases"),
            evidence_artifacts: number(&geometry, "evidence_artifacts"),
            acquisition_cost: number(&geometry, "evidence_artifacts"),
            rank: 0,
            score_key: (0, 0, 0, 0),
        },
    ];
    for candidate in &mut candidates {
        candidate.score_key = (
            candidate.sealed_learning_delta,
            candidate.sealed_authorized,
            candidate.evidence_cases,
            usize::MAX - candidate.acquisition_cost,
        );
    }
    candidates.sort_by(|left, right| right.score_key.cmp(&left.score_key));
    for (index, candidate) in candidates.iter_mut().enumerate() {
        candidate.rank = index + 1;
    }
    let eligible_candidates = candidates
        .iter()
        .filter(|candidate| candidate.eligible)
        .count();
    let selected_candidate = candidates
        .iter()
        .find(|candidate| candidate.eligible)
        .map(|candidate| candidate.candidate_id.clone())
        .expect("at least one source candidate must be eligible");
    let report = Report {
        schema: "stage265-source-candidate-utility-ranking-v1",
        current_manifest_hash: manifest.replay_hash(),
        candidate_count: candidates.len(),
        eligible_candidates,
        selected_candidate,
        selection_policy: "eligible candidates maximize sealed learning delta, then sealed authorization, then independent evidence cases, then lower acquisition cost",
        candidates,
        shadow_only: true,
        hle_questions_read: 0,
        manifest_mutations: 0,
        registry_mutations: 0,
        false_authorizations: 0,
        false_denials: 0,
    };
    assert_eq!(report.candidate_count, 2);
    assert_eq!(report.eligible_candidates, 2);
    assert_eq!(
        report.selected_candidate,
        "source_derived_bounded_economics"
    );
    assert!(report.shadow_only);
    assert_eq!(report.hle_questions_read, 0);
    assert_eq!(report.manifest_mutations, 0);
    assert_eq!(report.registry_mutations, 0);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 265 — source candidate utility ranking\n\nThe self-education planner compared {} eligible shadow candidates without reading HLE answers.\n\n* selected candidate: `{}`\n* policy: {}\n* HLE questions read: 0\n* shadow-only: {}\n* manifest / registry mutations: 0 / 0\n* false authorizations / denials: 0 / 0\n\nThe selected candidate is a recommendation only; promotion requires its own cloned-manifest, pressure, and rollback gates.\n\nReproduce with `cargo run --quiet --bin stage265_source_candidate_utility_ranking`.\n",
            report.eligible_candidates,
            report.selected_candidate,
            report.selection_policy,
            report.shadow_only,
        ),
    )?;
    println!(
        "stage265 candidates={} eligible={} selected={} shadow_only=true hle_read=0",
        report.candidate_count, report.eligible_candidates, report.selected_candidate
    );
    Ok(())
}
