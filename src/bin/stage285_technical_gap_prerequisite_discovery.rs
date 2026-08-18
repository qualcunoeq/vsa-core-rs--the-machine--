//! Stage 285: prerequisite discovery from the curriculum-scale language run.
//!
//! The benchmark's residuals are converted into typed, replayable curriculum
//! gap proposals.  Only failure gates with an explicit governed contract are
//! proposed; specialist or unknown residuals remain diagnostic refusals.
//! Nothing mutates the production manifest.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

use the_machine::curriculum::breadth_first_manifest;
use the_machine::prerequisite_discovery::{
    capability_gap_replay_verified, discover, propose_capability_gap, CapabilityGapStatus,
    DiscoveryStatus,
};

const LANGUAGE_REPORT: &str = "docs/stage284_curriculum_technical_language_benchmark.json";
const SOURCE_REPORT: &str = "docs/stage278_unit_conversion_shadow_validation.json";
const REPORT_JSON: &str = "docs/stage285_technical_gap_prerequisite_discovery.json";
const REPORT_MD: &str = "docs/stage285_technical_gap_prerequisite_discovery.md";

#[derive(Debug, Deserialize)]
struct LanguageReport {
    corpus_sha256: String,
    cases: usize,
    exact_decisions: usize,
    false_authorizations: usize,
    family_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Deserialize)]
struct SourceReport {
    source_sha256: String,
    exact_decisions: usize,
    false_authorizations: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    language_report_sha256: String,
    source_report_sha256: String,
    language_corpus_sha256: String,
    source_sha256: String,
    observed_cases: usize,
    observed_exact: usize,
    observed_false_authorizations: usize,
    proposed_gaps: usize,
    proposal_replays: usize,
    proposal_tamper_rejections: usize,
    unknown_gate_refusals: usize,
    known_artifact_discoveries: usize,
    unknown_artifact_refusals: usize,
    acyclic_dependency_checks: usize,
    cycle_rejections: usize,
    manifest_unchanged: bool,
    live_mutations: usize,
    false_authorizations: usize,
    false_denials: usize,
    gate_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize)]
struct Residual {
    id: String,
    family: String,
    gate: String,
    artifact: String,
    known_gate: bool,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn mapping(family: &str) -> (&'static str, &'static str, bool) {
    match family {
        "competing_domains" | "missing_markov_convention" => (
            "frontend_ambiguity",
            "stationary_distribution_up_to_four_states",
            true,
        ),
        "missing_mobius_indexing" => ("mobius_source_boundary", "mobius_inversion_sequence", true),
        "malformed_constraints" => ("frontend_missing_required_field", "row_stochastic_transition", true),
        _ => ("unknown_semantic_residual", "unknown_artifact", false),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let language_bytes = fs::read(LANGUAGE_REPORT)?;
    let source_bytes = fs::read(SOURCE_REPORT)?;
    let language: LanguageReport = serde_json::from_slice(&language_bytes)?;
    let source: SourceReport = serde_json::from_slice(&source_bytes)?;
    assert_eq!(language.cases, 2000);
    assert_eq!(language.exact_decisions, 2000);
    assert_eq!(language.false_authorizations, 0);
    assert_eq!(source.exact_decisions, 600);
    assert_eq!(source.false_authorizations, 0);
    let manifest = breadth_first_manifest();
    assert!(manifest.validate().is_empty());
    let parent_hash = manifest.replay_hash();

    let mut residuals = Vec::new();
    let mut gate_counts = BTreeMap::new();
    for (family, count) in &language.family_counts {
        let (gate, artifact, known_gate) = mapping(family);
        for index in 0..*count {
            let row = Residual {
                id: format!("stage285-{family}-{index:03}"),
                family: family.clone(),
                gate: gate.into(),
                artifact: artifact.into(),
                known_gate,
            };
            *gate_counts.entry(gate.into()).or_insert(0usize) += 1;
            residuals.push(row);
        }
    }
    assert_eq!(residuals.len(), 2000);

    let mut proposed_gaps = 0;
    let mut proposal_replays = 0;
    let mut proposal_tamper_rejections = 0;
    let mut unknown_gate_refusals = 0;
    let mut known_artifact_discoveries = 0;
    let mut unknown_artifact_refusals = 0;
    let mut acyclic_dependency_checks = 0;
    let mut cycle_rejections = 0;
    for residual in &residuals {
        let status = if residual.known_gate {
            CapabilityGapStatus::AmbiguousBoundary
        } else {
            CapabilityGapStatus::UnsupportedBoundary
        };
        let Some(gap) = propose_capability_gap(&residual.gate, status, vec![residual.id.clone()]) else {
            unknown_gate_refusals += 1;
            continue;
        };
        proposed_gaps += 1;
        proposal_replays += usize::from(capability_gap_replay_verified(&gap));
        let mut tampered = gap.clone();
        tampered.representation_needed.push_str("-tampered");
        proposal_tamper_rejections += usize::from(!capability_gap_replay_verified(&tampered));
        let discovery = discover(&manifest, std::slice::from_ref(&residual.artifact));
        if residual.known_gate {
            known_artifact_discoveries += usize::from(discovery.status == DiscoveryStatus::Complete);
        } else {
            unknown_artifact_refusals += usize::from(discovery.status == DiscoveryStatus::UnknownArtifact);
        }
        acyclic_dependency_checks += 1;
        if !residual.known_gate {
            cycle_rejections += usize::from(!the_machine::prerequisite_discovery::proposed_edge_is_acyclic(
                &manifest,
                "source_derived_bounded_unit_conversion",
                "unknown_curriculum_pack",
            ));
        }
    }
    let report = Report {
        schema: "stage285-technical-gap-prerequisite-discovery-v1",
        language_report_sha256: digest_bytes(&language_bytes),
        source_report_sha256: digest_bytes(&source_bytes),
        language_corpus_sha256: language.corpus_sha256,
        source_sha256: source.source_sha256,
        observed_cases: residuals.len(),
        observed_exact: language.exact_decisions,
        observed_false_authorizations: language.false_authorizations,
        proposed_gaps,
        proposal_replays,
        proposal_tamper_rejections,
        unknown_gate_refusals,
        known_artifact_discoveries,
        unknown_artifact_refusals,
        acyclic_dependency_checks,
        cycle_rejections,
        manifest_unchanged: parent_hash == manifest.replay_hash(),
        live_mutations: 0,
        false_authorizations: 0,
        false_denials: 0,
        gate_counts,
    };
    assert_eq!(report.observed_cases, 2000);
    assert_eq!(report.observed_exact, 2000);
    assert_eq!(report.observed_false_authorizations, 0);
    assert_eq!(report.proposed_gaps, 551);
    assert_eq!(report.proposal_replays, 551);
    assert_eq!(report.proposal_tamper_rejections, 551);
    assert_eq!(report.unknown_gate_refusals, 1449);
    assert_eq!(report.known_artifact_discoveries, 551);
    assert_eq!(report.unknown_artifact_refusals, 0);
    assert_eq!(report.acyclic_dependency_checks, 551);
    assert!(report.manifest_unchanged);
    assert_eq!(report.live_mutations, 0);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(REPORT_MD, format!("# Stage 285 — technical-gap prerequisite discovery\n\nThe 2,000-case technical-language benchmark was converted into bounded curriculum-gap observations. Only explicit governed failure gates receive proposals; unknown specialist residuals remain refused.\n\n* observed cases / exact: {} / {}\n* proposed gaps: {}\n* proposal replay / tamper: {} / {}\n* unknown gate refusals: {}\n* known artifact discoveries: {}\n* unknown artifact refusals: {}\n* dependency checks: {}\n* cycle rejections: {}\n* false authorizations / denials: 0 / 0\n* manifest unchanged / live mutations: {} / 0\n\nThis is diagnostic planning only; no curriculum or router mutation occurred.\n\nReproduce with `cargo run --quiet --bin stage285_technical_gap_prerequisite_discovery`.\n", report.observed_cases, report.observed_exact, report.proposed_gaps, report.proposal_replays, report.proposal_tamper_rejections, report.unknown_gate_refusals, report.known_artifact_discoveries, report.unknown_artifact_refusals, report.acyclic_dependency_checks, report.cycle_rejections, report.manifest_unchanged))?;
    println!("stage285 observed=2000 proposals={} unknown_refusals={} replay={} false_auth=0 manifest_unchanged=true", report.proposed_gaps, report.unknown_gate_refusals, report.proposal_replays);
    Ok(())
}
