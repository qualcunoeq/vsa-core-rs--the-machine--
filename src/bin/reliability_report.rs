//! Phase 7 reliability report.
//!
//! Turns the written reliability claims into a machine-checkable artifact:
//!
//! * drives a `VSABrain` through sustained ingestion under a declared
//!   [`MemoryBudget`] and verifies the caps actually hold;
//! * accounts the full resident footprint (entries, metadata, accumulators,
//!   centroids, indexes, conversation state) and checks it against the budget;
//! * emits the replay taxonomy (hash consistency / recomputation / independent
//!   verification) so a consumer cannot overstate a replay result;
//! * emits the guarantee registry (checked / enforced / assumed) so reported
//!   guarantees match the active configuration.
//!
//! It exits non-zero if an *enforced* guarantee is violated.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;

use the_machine::reliability::{
    account_conversation, guarantee_registry, guarantee_status_counts, projection_path,
    replay_catalogue, replay_class_counts, GuaranteeStatus, MemoryBudget, MemoryReport, ReplayClass,
};
use the_machine::{Hypervector, VSABrain};

const JSON_PATH: &str = "docs/phase7_reliability_v1.report.json";
const MARKDOWN_PATH: &str = "docs/phase7_reliability_v1.md";
const SUSTAINED_INSERTS: usize = 20_000;

#[derive(Serialize)]
struct ReliabilityReport {
    schema: &'static str,
    sustained_inserts: usize,
    memory_budget: BudgetView,
    memory_report: MemoryReport,
    memory_within_budget: bool,
    cluster_cap_respected: bool,
    entry_cap_respected: bool,
    transient_cap_respected: bool,
    projection_path: &'static str,
    replay_classes: HashMap<String, usize>,
    guarantees: Vec<GuaranteeView>,
    guarantee_counts: HashMap<String, usize>,
}

#[derive(Serialize)]
struct BudgetView {
    max_clusters: usize,
    max_entries_per_cluster: usize,
    max_transient_clusters: usize,
    max_entries_per_transient_cluster: usize,
    max_total_bytes: usize,
}

#[derive(Serialize)]
struct GuaranteeView {
    id: String,
    statement: String,
    status: &'static str,
    witness: String,
    configuration: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("report serializes"))
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let budget = MemoryBudget {
        max_clusters: 512,
        max_entries_per_cluster: 64,
        max_transient_clusters: 256,
        max_entries_per_transient_cluster: 32,
        max_total_bytes: 64 * 1024 * 1024,
    };

    let mut brain = VSABrain::new(0.30);
    brain.memory_budget = budget;

    // Explore several well-separated "families" so the brain actually forms
    // more than one cluster and exercises accumulator/association accounting.
    const FAMILIES: u64 = 24;
    for i in 0..SUSTAINED_INSERTS {
        let family = (i as u64) % FAMILIES;
        let mut bits = [0u64; 160];
        // Family-defining sparse pattern (deterministic per family).
        let mut x = family.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        for word in bits.iter_mut() {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            *word = x;
        }
        // A little per-observation variation within the family.
        bits[i % 160] ^= (i as u64).rotate_left((i % 63) as u32);
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), format!("doc-{}", i % 97));
        brain.add_to_dejavu_db(Hypervector { bits }, &format!("obs-{i}"), metadata);

        if i % 4 == 0 {
            let mut tb = [0u64; 160];
            tb[(i * 5) % 160] = 0xABCD_EF01_2345_6789;
            brain.add_transient_fact(
                Hypervector { bits: tb },
                &format!("t-{i}"),
                HashMap::new(),
            );
        }
    }
    // Rebuild the live indexer so its entries are part of the accounting.
    brain.rebuild_indexer();

    let cluster_cap_respected = brain.dejavu_clusters.len() <= budget.max_clusters;
    let entry_cap_respected = brain
        .dejavu_clusters
        .iter()
        .all(|c| c.entries.len() <= budget.max_entries_per_cluster);
    let transient_cap_respected = brain.transient_clusters.len() <= budget.max_transient_clusters
        && brain
            .transient_clusters
            .iter()
            .all(|c| c.entries.len() <= budget.max_entries_per_transient_cluster);

    let mut memory_report = brain.memory_report();
    // A representative conversation-state contribution, so the accounting is
    // shown to include conversation state rather than only the brain.
    account_conversation(&mut memory_report, 3, 120, 1, 24);
    let memory_within_budget = memory_report.within_budget();

    let replay_counts = replay_class_counts(&replay_catalogue())
        .into_iter()
        .map(|(class, count)| (class.label().to_string(), count))
        .collect::<HashMap<_, _>>();

    let guarantees = guarantee_registry()
        .into_iter()
        .map(|g| GuaranteeView {
            id: g.id,
            statement: g.statement,
            status: g.status.label(),
            witness: g.witness,
            configuration: g.configuration,
        })
        .collect::<Vec<_>>();
    let guarantee_counts = guarantee_status_counts(&guarantee_registry())
        .into_iter()
        .map(|(status, count)| (status.label().to_string(), count))
        .collect::<HashMap<_, _>>();

    let report = ReliabilityReport {
        schema: "phase7-reliability-v1",
        sustained_inserts: SUSTAINED_INSERTS,
        memory_budget: BudgetView {
            max_clusters: budget.max_clusters,
            max_entries_per_cluster: budget.max_entries_per_cluster,
            max_transient_clusters: budget.max_transient_clusters,
            max_entries_per_transient_cluster: budget.max_entries_per_transient_cluster,
            max_total_bytes: budget.max_total_bytes,
        },
        memory_report: memory_report.clone(),
        memory_within_budget,
        cluster_cap_respected,
        entry_cap_respected,
        transient_cap_respected,
        projection_path: projection_path().label(),
        replay_classes: replay_counts,
        guarantees,
        guarantee_counts,
    };

    let report_json = serde_json::to_string_pretty(&report)?;
    fs::write(JSON_PATH, format!("{report_json}\n"))?;

    let enforced = report
        .guarantee_counts
        .get("enforced")
        .copied()
        .unwrap_or(0);
    let checked = report
        .guarantee_counts
        .get("checked")
        .copied()
        .unwrap_or(0);
    let assumed = report
        .guarantee_counts
        .get("assumed")
        .copied()
        .unwrap_or(0);

    let markdown = format!(
        "# Phase 7 — reliability report\n\n\
Generated by `cargo run --bin reliability_report`. Sustained ingestion of \
{inserts} observations under a declared memory budget, plus the replay and \
guarantee registries.\n\n\
## Memory\n\n\
* budget: clusters {bc}, entries/cluster {be}, transient clusters {btc}, \
entries/transient {bte}, total {budget_mib} MiB\n\
* clusters: {clusters} (cap respected: {cc})\n\
* entries: {entries} (cap respected: {ec})\n\
* transient clusters: {tc} (cap respected: {trc})\n\
* accounted total: {total} bytes ({total_mib:.2} MiB), within budget: {within}\n\
* terms — entries {entry_b}, metadata {meta_b}, accumulators {acc_b}, \
centroids {cent_b}, associations {assoc_b}, experiences {exp_b}, indexes \
{index_b}, conversation {conv_b}\n\n\
## Projection telemetry\n\n\
* measured path: `{path}`\n\n\
## Replay taxonomy\n\n\
* hash consistency: {hash}\n\
* recomputation: {recomp}\n\
* independent verification: {indep}\n\n\
## Guarantees\n\n\
* enforced: {enforced}\n\
* checked: {checked}\n\
* assumed: {assumed}\n\
* report SHA-256: `{hash_sum}`\n",
        inserts = SUSTAINED_INSERTS,
        bc = budget.max_clusters,
        be = budget.max_entries_per_cluster,
        btc = budget.max_transient_clusters,
        bte = budget.max_entries_per_transient_cluster,
        budget_mib = budget.max_total_bytes / (1024 * 1024),
        clusters = memory_report.cluster_count,
        cc = cluster_cap_respected,
        entries = memory_report.entry_count,
        ec = entry_cap_respected,
        tc = memory_report.transient_cluster_count,
        trc = transient_cap_respected,
        total = memory_report.total_bytes,
        total_mib = memory_report.total_bytes as f64 / (1024.0 * 1024.0),
        within = memory_within_budget,
        entry_b = memory_report.entry_bytes,
        meta_b = memory_report.metadata_bytes,
        acc_b = memory_report.accumulator_bytes,
        cent_b = memory_report.centroid_bytes,
        assoc_b = memory_report.association_bytes,
        exp_b = memory_report.experience_bytes,
        index_b = memory_report.index_bytes,
        conv_b = memory_report.conversation_bytes,
        path = projection_path().label(),
        hash = report.replay_classes.get("hash_consistency").copied().unwrap_or(0),
        recomp = report.replay_classes.get("recomputation").copied().unwrap_or(0),
        indep = report.replay_classes.get("independent_verification").copied().unwrap_or(0),
        enforced = enforced,
        checked = checked,
        assumed = assumed,
        hash_sum = digest(&report_json),
    );
    fs::write(MARKDOWN_PATH, markdown)?;

    eprintln!(
        "inserts={SUSTAINED_INSERTS} clusters={} entries={} total_bytes={} within_budget={memory_within_budget} caps=[{cluster_cap_respected},{entry_cap_respected},{transient_cap_respected}] enforced={enforced} checked={checked} assumed={assumed} report={JSON_PATH}",
        memory_report.cluster_count,
        memory_report.entry_count,
        memory_report.total_bytes,
    );

    // Fail if an enforced guarantee is violated.
    if !memory_within_budget {
        return Err(format!(
            "accounted memory {} exceeds budget {}",
            memory_report.total_bytes, memory_report.budget_bytes
        )
        .into());
    }
    if !cluster_cap_respected || !entry_cap_respected || !transient_cap_respected {
        return Err("a declared memory cap was exceeded under sustained ingestion".into());
    }
    // Every replay class must be represented; a missing one would mean the
    // taxonomy has collapsed two strengths into one.
    for class in [
        ReplayClass::HashConsistency,
        ReplayClass::Recomputation,
        ReplayClass::IndependentVerification,
    ] {
        if report.replay_classes.get(class.label()).copied().unwrap_or(0) == 0 {
            return Err(format!("replay class {} is unrepresented", class.label()).into());
        }
    }
    // A guarantee marked enforced must name a witness.
    for guarantee in guarantee_registry() {
        if guarantee.status == GuaranteeStatus::Enforced && guarantee.witness.trim().is_empty() {
            return Err(format!("enforced guarantee {} has no witness", guarantee.id).into());
        }
    }
    Ok(())
}
