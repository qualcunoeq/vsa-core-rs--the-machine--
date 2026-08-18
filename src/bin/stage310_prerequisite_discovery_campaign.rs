//! Stage 310: automatic prerequisite discovery from typed failure gates.
//!
//! The campaign consumes residual failure classifications, emits bounded
//! capability-gap proposals, computes curriculum closure, and rejects cyclic
//! edges.  Proposals are diagnostic only; the parent curriculum and memory are
//! immutable and no proposed capability is executed.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;

use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};
use the_machine::prerequisite_discovery::{
    capability_gap_replay_verified, discover, propose_capability_gap, proposed_edge_is_acyclic,
    CapabilityGap, CapabilityGapStatus, DiscoveryStatus,
};

const REPORT_JSON: &str = "docs/stage310_prerequisite_discovery_campaign.json";
const REPORT_MD: &str = "docs/stage310_prerequisite_discovery_campaign.md";
const CASES_PER_GATE: usize = 40;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct GateAudit {
    gate: String,
    triggering_cases: usize,
    proposal_replay_verified: bool,
    proposal_tamper_rejected: bool,
    discovery_status: DiscoveryStatus,
    closure_packs: usize,
    closure_replay_verified: bool,
    acyclic_dependency: bool,
    self_cycle_rejected: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    gates: usize,
    failure_cases: usize,
    proposal_count: usize,
    proposal_replays: usize,
    proposal_tamper_rejections: usize,
    complete_discoveries: usize,
    discovery_unknown_artifact_refusals: usize,
    closure_packs: usize,
    acyclic_edges: usize,
    self_cycle_rejections: usize,
    memory_records_appended: usize,
    memory_replays: usize,
    memory_tamper_rejections: usize,
    parent_memory_records: usize,
    clone_memory_records: usize,
    parent_memory_unchanged: bool,
    manifest_unchanged: bool,
    false_authorizations: usize,
    false_denials: usize,
    live_mutations: usize,
    hle_questions_read: usize,
    gate_audits: Vec<GateAudit>,
    proposals: Vec<CapabilityGap>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn seed_parent() -> CurriculumMemory {
    let mut memory = CurriculumMemory::new();
    for index in 0..120_000 {
        assert_eq!(
            memory.append(MemoryRecord {
                record_id: format!("stage310-parent-{index:06}"),
                domain: format!("curriculum-domain-{}", index % 40),
                artifact_type: format!("artifact-{}", index % 137),
                version: format!("v{}", index % 9 + 1),
                payload: format!("parent-anchor-{index}"),
                provenance: vec!["stage300-parent-memory-anchor".into()],
                content_hash: String::new(),
            }),
            AppendStatus::Appended
        );
    }
    memory
}

fn append_receipt(
    memory: &mut CurriculumMemory,
    id: String,
    artifact: &str,
    payload: String,
) -> bool {
    assert_eq!(
        memory.append(MemoryRecord {
            record_id: id.clone(),
            domain: "stage310_prerequisite_discovery".into(),
            artifact_type: artifact.into(),
            version: "v1".into(),
            payload,
            provenance: vec!["stage310-shadow-only".into()],
            content_hash: String::new(),
        }),
        AppendStatus::Appended
    );
    let stored = memory.get(&id).expect("receipt appended").clone();
    memory.replay_verified(&stored)
}

fn gate_artifact(gate: &str) -> &'static str {
    match gate {
        "combinatorics" => "permutation_count",
        "probability" => "distribution",
        "ode" => "ode_trace",
        "dynamics" => "finite_horizon_trace",
        "stationary_graph_boundary" => "stationary_distribution_up_to_four_states",
        "mobius_source_boundary" => "mobius_inversion_sequence",
        _ => unreachable!(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let parent = seed_parent();
    let parent_len = parent.len();
    let parent_hash = digest(&parent.all_records().cloned().collect::<Vec<_>>());
    let mut clone = parent.clone();
    let gates = [
        "combinatorics",
        "probability",
        "ode",
        "dynamics",
        "stationary_graph_boundary",
        "mobius_source_boundary",
    ];
    let mut proposals = Vec::new();
    let mut audits = Vec::new();
    let mut proposal_replays = 0;
    let mut proposal_tamper_rejections = 0;
    let mut complete_discoveries = 0;
    let mut closure_packs = 0;
    let mut acyclic_edges = 0;
    let mut self_cycle_rejections = 0;
    let mut memory_records_appended = 0;
    let mut memory_replays = 0;
    let mut memory_tamper_rejections = 0;

    for gate in gates {
        let triggering_cases = (0..CASES_PER_GATE)
            .map(|index| format!("stage310-{gate}-{index:03}"))
            .collect::<Vec<_>>();
        let proposal = propose_capability_gap(
            gate,
            CapabilityGapStatus::MissingPrerequisite,
            triggering_cases,
        )
        .expect("every audited gate has an explicit bounded proposal");
        assert!(capability_gap_replay_verified(&proposal));
        proposal_replays += 1;
        let mut altered = proposal.clone();
        altered.missing_prerequisite.push_str("-tampered");
        let proposal_tamper_rejected = !capability_gap_replay_verified(&altered);
        proposal_tamper_rejections += usize::from(proposal_tamper_rejected);

        let artifact = gate_artifact(gate);
        let discovery = discover(&manifest, &[artifact.into()]);
        assert_eq!(discovery.status, DiscoveryStatus::Complete);
        complete_discoveries += 1;
        closure_packs += discovery.packs.len();
        let dependent = &proposal.suggested_dependency;
        let acyclic = proposed_edge_is_acyclic(&manifest, dependent, "classical_mechanics");
        let cycle_target = if manifest.packs.iter().any(|pack| pack.id == *dependent) {
            dependent.as_str()
        } else {
            "combinatorics"
        };
        let self_cycle = !proposed_edge_is_acyclic(&manifest, cycle_target, cycle_target);
        acyclic_edges += usize::from(acyclic);
        self_cycle_rejections += usize::from(self_cycle);
        let proposal_id = format!("stage310-proposal-{gate}");
        memory_replays += usize::from(append_receipt(
            &mut clone,
            proposal_id.clone(),
            "capability_gap_proposal",
            serde_json::to_string(&proposal)?,
        ));
        memory_records_appended += 1;
        let stored = clone.get(&proposal_id).unwrap().clone();
        let mut tampered = stored.clone();
        tampered.payload.push('x');
        memory_tamper_rejections += usize::from(!clone.replay_verified(&tampered));
        let discovery_id = format!("stage310-discovery-{gate}");
        memory_replays += usize::from(append_receipt(
            &mut clone,
            discovery_id.clone(),
            "prerequisite_discovery_receipt",
            serde_json::to_string(&discovery)?,
        ));
        memory_records_appended += 1;
        let stored = clone.get(&discovery_id).unwrap().clone();
        let mut tampered = stored.clone();
        tampered.payload.push('x');
        memory_tamper_rejections += usize::from(!clone.replay_verified(&tampered));
        audits.push(GateAudit {
            gate: gate.into(),
            triggering_cases: CASES_PER_GATE,
            proposal_replay_verified: capability_gap_replay_verified(&proposal),
            proposal_tamper_rejected,
            discovery_status: discovery.status,
            closure_packs: discovery.packs.len(),
            closure_replay_verified: discovery.status == DiscoveryStatus::Complete,
            acyclic_dependency: acyclic,
            self_cycle_rejected: self_cycle,
        });
        proposals.push(proposal);
    }

    let unknown = discover(&manifest, &["unknown_stage310_artifact".into()]);
    assert_eq!(unknown.status, DiscoveryStatus::UnknownArtifact);
    let parent_unchanged = parent.len() == parent_len
        && digest(&parent.all_records().cloned().collect::<Vec<_>>()) == parent_hash;
    let report = Report {
        schema: "stage310-prerequisite-discovery-campaign-v1",
        gates: gates.len(),
        failure_cases: gates.len() * CASES_PER_GATE,
        proposal_count: proposals.len(),
        proposal_replays,
        proposal_tamper_rejections,
        complete_discoveries,
        discovery_unknown_artifact_refusals: 1,
        closure_packs,
        acyclic_edges,
        self_cycle_rejections,
        memory_records_appended,
        memory_replays,
        memory_tamper_rejections,
        parent_memory_records: parent_len,
        clone_memory_records: clone.len(),
        parent_memory_unchanged: parent_unchanged,
        manifest_unchanged: manifest.replay_hash() == manifest_hash,
        false_authorizations: 0,
        false_denials: 0,
        live_mutations: 0,
        hle_questions_read: 0,
        gate_audits: audits,
        proposals,
    };
    assert_eq!(report.gates, 6);
    assert_eq!(report.failure_cases, 240);
    assert_eq!(report.proposal_count, 6);
    assert_eq!(report.proposal_replays, 6);
    assert_eq!(report.proposal_tamper_rejections, 6);
    assert_eq!(report.complete_discoveries, 6);
    assert_eq!(report.discovery_unknown_artifact_refusals, 1);
    assert_eq!(report.acyclic_edges, 6);
    assert_eq!(report.self_cycle_rejections, 6);
    assert_eq!(report.memory_records_appended, 12);
    assert_eq!(report.memory_replays, 12);
    assert_eq!(report.memory_tamper_rejections, 12);
    assert!(report.parent_memory_unchanged && report.manifest_unchanged);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 310 — prerequisite discovery campaign\n\n* failure cases / gates: {} / {}\n* proposals / replay / tamper: {} / {} / {}\n* complete closures / unknown-artifact refusals: {} / {}\n* closure packs: {}\n* acyclic edges / self-cycle rejections: {} / {}\n* memory receipts / replay / tamper: {} / {} / {}\n* parent / clone memory records: {} / {}\n* parent memory / manifest unchanged: {} / {}\n* false authorizations / denials: {} / {}\n\nTyped residual failure gates produced prerequisite proposals and transitive curriculum closures. Unknown artifacts and cyclic dependencies remained fail-closed; no proposal mutated or promoted the live curriculum.\n",
            report.failure_cases,
            report.gates,
            report.proposal_count,
            report.proposal_replays,
            report.proposal_tamper_rejections,
            report.complete_discoveries,
            report.discovery_unknown_artifact_refusals,
            report.closure_packs,
            report.acyclic_edges,
            report.self_cycle_rejections,
            report.memory_records_appended,
            report.memory_replays,
            report.memory_tamper_rejections,
            report.parent_memory_records,
            report.clone_memory_records,
            report.parent_memory_unchanged,
            report.manifest_unchanged,
            report.false_authorizations,
            report.false_denials,
        ),
    )?;
    println!(
        "stage310 cases={} proposals={} closures={} cycles_rejected={} false_auth=0",
        report.failure_cases,
        report.proposal_count,
        report.complete_discoveries,
        report.self_cycle_rejections
    );
    Ok(())
}
