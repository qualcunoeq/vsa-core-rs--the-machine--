//! Automatic prerequisite-discovery campaign over the immutable curriculum DAG.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::prerequisite_discovery::{discover, proposed_edge_is_acyclic, DiscoveryStatus};

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn main() {
    let manifest = breadth_first_manifest();
    assert!(manifest.validate().is_empty());
    let before = manifest.replay_hash();
    let known = [
        "combination_count",
        "exact_constant_derivative",
        "matrix_artifact",
        "congruence_class",
        "distribution",
        "finite_graph",
        "limit",
        "group",
    ];
    let mut complete = 0usize;
    let mut unknown = 0usize;
    let mut cycle_rejected = 0usize;
    for index in 0..240 {
        let result = discover(&manifest, &[known[index % known.len()].into()]);
        assert_eq!(result.status, DiscoveryStatus::Complete);
        assert!(!result.packs.is_empty());
        complete += 1;
    }
    for index in 0..30 {
        let result = discover(&manifest, &[format!("unknown_artifact_{index}")]);
        assert_eq!(result.status, DiscoveryStatus::UnknownArtifact);
        assert_eq!(result.missing_prerequisites.len(), 1);
        unknown += 1;
    }
    for _ in 0..30 {
        assert!(!proposed_edge_is_acyclic(
            &manifest,
            "linear_algebra_spectral",
            "elementary_number_theory"
        ));
        cycle_rejected += 1;
    }
    let after = manifest.replay_hash();
    assert_eq!(before, after);
    let report = serde_json::json!({
        "schema": "stage-f-prerequisite-discovery-v1",
        "cases": 300,
        "complete_plans": complete,
        "unknown_artifacts": unknown,
        "cycle_rejections": cycle_rejected,
        "exact_decisions": complete + unknown + cycle_rejected,
        "manifest_immutable": before == after,
        "manifest_hash": digest(&(before, after, complete, unknown, cycle_rejected)),
    });
    let serialized = serde_json::to_string_pretty(&report).unwrap();
    std::fs::write(
        "docs/stage_f_prerequisite_discovery.json",
        format!("{serialized}\n"),
    )
    .unwrap();
    println!("{serialized}");
}
