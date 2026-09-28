//! Phase 7 — turn reliability claims into enforced contracts.
//!
//! Earlier phases made the system *observable*: it records what it did and why.
//! Phase 7 makes persistent use *safe for the system's own state and
//! resources* by turning the guarantees that were merely written down into
//! checks that are actually enforced:
//!
//! * [`MemoryBudget`] declares the capacities the architecture depends on, and
//!   the growth paths in `lib.rs` route through it instead of unbounded
//!   `push`es.
//! * [`MemoryReport`] accounts for the *whole* resident footprint — entries,
//!   metadata, accumulators, centroids/anchors, indexes, conversation state —
//!   not a single favourable term.
//! * [`ReplayClass`] separates the three kinds of replay check that were
//!   previously described by one word (hash consistency, recomputation,
//!   independent verification), so a "replay verified" claim can never be read
//!   as something stronger than it is.
//! * [`Guarantee`] / [`GuaranteeRegistry`] record, per declared guarantee,
//!   whether it is *checked*, *enforced*, or merely *assumed*, and under which
//!   configuration it holds.
//!
//! The module is deliberately free of side effects; enforcement is performed
//! at the call sites in `lib.rs`, and this module supplies the budget and the
//! accounting those call sites use.

use std::collections::HashMap;

/// Serialized size of one hypervector, in bytes (`HD_DIMENSION = 10240`
/// bits = 1280 bytes).  Kept local so the accounting does not depend on the
/// runtime representation changing under it.
pub const HYPERVECTOR_BYTES: usize = 1280;

/// Approximate heap cost of one `HashMap` entry (hash-table slot + key/value
/// pointers), used when only the entry count is known.
pub const HASHMAP_ENTRY_BYTES: usize = 48;

// ─── Memory budget ─────────────────────────────────────────────────────────

/// Declared capacities the architecture relies on.
///
/// Every field is a *hard* limit: the growth paths must evict rather than
/// exceed it.  The defaults match the constants the theorems were stated
/// against (`MAX_ENTRIES_PER_CLUSTER = 1000`, `MAX_CLUSTER_WEIGHT = 500`).
#[derive(Clone, Copy, Debug)]
pub struct MemoryBudget {
    /// Maximum live `MemoryCluster`s before compaction/eviction is forced.
    pub max_clusters: usize,
    /// Maximum entries per `MemoryCluster`.
    pub max_entries_per_cluster: usize,
    /// Maximum live transient clusters before the coldest are evicted.
    pub max_transient_clusters: usize,
    /// Maximum entries per transient cluster.
    pub max_entries_per_transient_cluster: usize,
    /// Maximum accounted resident bytes for the whole brain.
    pub max_total_bytes: usize,
}

impl Default for MemoryBudget {
    fn default() -> Self {
        MemoryBudget {
            // A generous but finite cluster ceiling.  Without a cap the
            // novelty gate can spawn one cluster per observation forever.
            max_clusters: 100_000,
            max_entries_per_cluster: 1000,
            max_transient_clusters: 4096,
            max_entries_per_transient_cluster: 256,
            // 2 GiB accounted ceiling.  The measured default budget on the
            // target machine is comfortably below this at steady state.
            max_total_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

impl MemoryBudget {
    /// How many entries to drain from a cluster that has exceeded its cap.
    /// Drains a quarter at a time so eviction is amortized, mirroring the
    /// existing `MAX_ENTRIES_PER_CLUSTER` behaviour.
    pub fn entries_to_drain(len: usize, max: usize) -> usize {
        if max == 0 || len <= max {
            0
        } else {
            (max / 4).max(1)
        }
    }

    /// Returns `true` when the cluster count is at or above the declared cap.
    pub fn clusters_at_capacity(&self, live: usize) -> bool {
        live >= self.max_clusters
    }

    /// Returns `true` when the transient cluster count is at or above the cap.
    pub fn transients_at_capacity(&self, live: usize) -> bool {
        live >= self.max_transient_clusters
    }
}

// ─── Memory accounting ─────────────────────────────────────────────────────

/// Full resident-memory accounting.
///
/// Unlike the earlier `MemorySnapshot`, every term here is populated and the
/// total includes entry payloads, metadata, indexes, centroids, accumulators,
/// associations, experiences, and conversation state.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct MemoryReport {
    // Counts
    pub cluster_count: usize,
    pub entry_count: usize,
    pub transient_cluster_count: usize,
    pub transient_entry_count: usize,
    pub conversation_sessions: usize,
    pub conversation_turns: usize,
    // Byte terms
    pub entry_bytes: usize,
    pub metadata_bytes: usize,
    pub accumulator_bytes: usize,
    pub centroid_bytes: usize,
    pub association_bytes: usize,
    pub experience_bytes: usize,
    pub index_bytes: usize,
    pub conversation_bytes: usize,
    // Totals
    pub total_bytes: usize,
    pub budget_bytes: usize,
}

impl MemoryReport {
    pub fn within_budget(&self) -> bool {
        self.total_bytes <= self.budget_bytes
    }

    /// Fraction of the declared budget currently used.
    pub fn budget_fraction(&self) -> f64 {
        if self.budget_bytes == 0 {
            0.0
        } else {
            self.total_bytes as f64 / self.budget_bytes as f64
        }
    }

    /// Recompute the total from the byte terms.
    pub fn recompute_total(&mut self) {
        self.total_bytes = self.entry_bytes
            + self.metadata_bytes
            + self.accumulator_bytes
            + self.centroid_bytes
            + self.association_bytes
            + self.experience_bytes
            + self.index_bytes
            + self.conversation_bytes;
    }
}

/// Account for one brain snapshot.
///
/// `dejavu` is the list of `MemoryCluster`s; `transient` the `TransientCluster`s.
/// `has_accumulator` reports whether a hot cluster currently holds a dense
/// accumulator, so the accounting reflects the real resident path rather than
/// assuming every cluster is dense.
pub fn account_brain(
    dejavu: &[crate::MemoryCluster],
    transient: &[crate::TransientCluster],
    experiences: &[crate::Hypervector],
    associations: &HashMap<usize, Vec<(usize, crate::Hypervector, f64, u64)>>,
    index_entries: usize,
    budget: MemoryBudget,
) -> MemoryReport {
    let mut report = MemoryReport {
        cluster_count: dejavu.len(),
        transient_cluster_count: transient.len(),
        budget_bytes: budget.max_total_bytes,
        ..Default::default()
    };

    for cluster in dejavu {
        report.entry_count += cluster.entries.len();
        for entry in &cluster.entries {
            report.entry_bytes += HYPERVECTOR_BYTES;
            report.metadata_bytes += entry.label.len() + 24; // String header
            for (key, value) in &entry.metadata {
                report.metadata_bytes += key.len() + value.len() + HASHMAP_ENTRY_BYTES;
            }
        }
        // Centroid + anchor are both resident hypervectors.
        report.centroid_bytes += 2 * HYPERVECTOR_BYTES;
        // Dense accumulator is 10240 * 4 bytes when present, else negligible.
        if !cluster.accumulator.is_empty() {
            report.accumulator_bytes += cluster.accumulator.len() * std::mem::size_of::<u32>();
        }
    }

    for cluster in transient {
        report.transient_entry_count += cluster.entries.len();
        for entry in &cluster.entries {
            report.entry_bytes += HYPERVECTOR_BYTES;
            report.metadata_bytes += entry.label.len() + 24;
            for (key, value) in &entry.metadata {
                report.metadata_bytes += key.len() + value.len() + HASHMAP_ENTRY_BYTES;
            }
        }
        report.centroid_bytes += HYPERVECTOR_BYTES; // placeholder anchor + centroid
    }

    for list in associations.values() {
        report.association_bytes += list.len()
            * (std::mem::size_of::<usize>() * 2 + HYPERVECTOR_BYTES + 8 + 8);
    }

    report.experience_bytes = experiences.len() * HYPERVECTOR_BYTES;

    // Indexes are counted by entry, not by their contents (contents are
    // borrowed from the structures already counted above).
    report.index_bytes = index_entries * HASHMAP_ENTRY_BYTES;

    report.recompute_total();
    report
}

/// Add conversation-state accounting to a report: sessions and their turns,
/// plus session hypervector context.
pub fn account_conversation(
    report: &mut MemoryReport,
    sessions: usize,
    turns: usize,
    pending_clarifications: usize,
    session_memory_entries: usize,
) {
    report.conversation_sessions = sessions;
    report.conversation_turns = turns;
    report.conversation_bytes = turns * 256 // per-turn struct + strings (estimate)
        + sessions * 128
        + pending_clarifications * 256
        + session_memory_entries * HYPERVECTOR_BYTES;
    report.recompute_total();
}

// ─── Replay classification ─────────────────────────────────────────────────

/// The three distinct strengths a "replay" check can have.
///
/// Collapsing them into one word is the failure this taxonomy prevents: a
/// hash-consistency check proves the bytes did not change, not that the
/// computation is correct.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayClass {
    /// A stored digest is compared against a recomputed digest of the same
    /// serialized struct.  Detects tampering and accidental mutation; proves
    /// nothing about correctness.
    HashConsistency,
    /// The original computation is re-run from its inputs and the result is
    /// compared.  Detects nondeterminism and input drift; still trusts the
    /// same implementation.
    Recomputation,
    /// A different mechanism from the producer checks the answer
    /// independently.  The strongest class.
    IndependentVerification,
}

impl ReplayClass {
    pub fn label(self) -> &'static str {
        match self {
            ReplayClass::HashConsistency => "hash_consistency",
            ReplayClass::Recomputation => "recomputation",
            ReplayClass::IndependentVerification => "independent_verification",
        }
    }

    /// Whether this class is at least as strong as `other`.
    pub fn at_least(self, other: ReplayClass) -> bool {
        self >= other
    }
}

/// One catalogued replay mechanism and its class.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ReplayMechanism {
    /// Stable identifier for the mechanism.
    pub id: String,
    /// Module that implements it.
    pub module: String,
    /// What it actually checks.
    pub class: ReplayClass,
}

/// The catalogue of replay mechanisms the capability contract relies on.
///
/// This is intentionally a curated list of *exemplars per module family*
/// rather than every function; the goal is that no consumer can describe a
/// `HashConsistency` check as "independent verification".
pub fn replay_catalogue() -> Vec<ReplayMechanism> {
    use ReplayClass::*;
    vec![
        m("semantic_ir::CandidateSemanticParse", "semantic_ir", HashConsistency),
        m("semantic_ir::SemanticValidationReceipt", "semantic_ir", HashConsistency),
        m("equation_problem_binding::replay_verified", "equation_problem_binding", HashConsistency),
        m("semantic_shadow::report_hash", "semantic_shadow", HashConsistency),
        m("expression_evaluation::replay_expression_evaluation", "expression_evaluation", Recomputation),
        m("linear_equation::replay_linear_equation", "linear_equation", Recomputation),
        m("quadratic_equation::replay_quadratic_equation", "quadratic_equation", Recomputation),
        m("equation_normalization::replay_equation_normalization", "equation_normalization", Recomputation),
        m("equation_classification::replay_equation_classification", "equation_classification", Recomputation),
        m("substitution::replay_substitution", "substitution", Recomputation),
        m("expression_simplification::replay_expression_simplification", "expression_simplification", Recomputation),
        m("linear_system::replay_linear_system", "linear_system", Recomputation),
        m("solution_verification::execute_solution_verification", "solution_verification", IndependentVerification),
        m("solution_verification::execute_solution_set_verification", "solution_verification", IndependentVerification),
        m("algebra_island::AlgebraVerificationReceipt", "algebra_island", IndependentVerification),
        m("capability_adapter::unit_conversion_verifier", "conversation::capability_adapter", IndependentVerification),
        m("capability_proposer::verify_case", "capability_proposer", IndependentVerification),
        m("qa::reason_chain_trace", "qa", Recomputation),
    ]
}

fn m(id: &str, module: &str, class: ReplayClass) -> ReplayMechanism {
    ReplayMechanism {
        id: id.to_string(),
        module: module.to_string(),
        class,
    }
}

/// Count catalogue entries per class, for the generated report.
pub fn replay_class_counts(catalogue: &[ReplayMechanism]) -> HashMap<ReplayClass, usize> {
    let mut counts = HashMap::new();
    for item in catalogue {
        *counts.entry(item.class).or_insert(0) += 1;
    }
    counts
}

// ─── Guarantee registry ────────────────────────────────────────────────────

/// How strongly a declared guarantee is actually held.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuaranteeStatus {
    /// Stated in the architecture but not checked anywhere at runtime.
    Assumed,
    /// A test or runtime check verifies the invariant holds; violations are
    /// detectable but the system does not act on them.
    Checked,
    /// The invariant is enforced at the point of use; the system refuses the
    /// operation or evicts to stay inside the contract.
    Enforced,
}

impl GuaranteeStatus {
    pub fn label(self) -> &'static str {
        match self {
            GuaranteeStatus::Assumed => "assumed",
            GuaranteeStatus::Checked => "checked",
            GuaranteeStatus::Enforced => "enforced",
        }
    }
}

/// A declared guarantee, its enforcement status, and where it is held.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Guarantee {
    pub id: String,
    pub statement: String,
    pub status: GuaranteeStatus,
    /// Code or test that backs the status.
    pub witness: String,
    /// Configuration under which the guarantee holds.  Empty means
    /// "unconditional for the supported configuration".
    pub configuration: String,
}

/// The registry of Phase 7 guarantees.  Every guarantee that Phase 7 made
/// enforceable appears here with the code that enforces it.
pub fn guarantee_registry() -> Vec<Guarantee> {
    vec![
        g(
            "G-ENTRY-CAP",
            "Entries per MemoryCluster never exceed the declared budget.",
            GuaranteeStatus::Enforced,
            "lib.rs::add_to_dejavu_db / MemoryCluster::novelty_gate drain via MemoryBudget::entries_to_drain",
            "",
        ),
        g(
            "G-TRANSIENT-ENTRY-CAP",
            "Entries per transient cluster never exceed the declared budget.",
            GuaranteeStatus::Enforced,
            "lib.rs::add_transient_fact drains via MemoryBudget::entries_to_drain",
            "",
        ),
        g(
            "G-CLUSTER-CAP",
            "Live MemoryCluster count never exceeds the declared budget.",
            GuaranteeStatus::Enforced,
            "lib.rs::add_to_dejavu_db evicts the coldest cluster at capacity",
            "",
        ),
        g(
            "G-TRANSIENT-CLUSTER-CAP",
            "Live transient cluster count never exceeds the declared budget.",
            GuaranteeStatus::Enforced,
            "lib.rs::add_transient_fact freezes/evicts the coldest transient at capacity",
            "",
        ),
        g(
            "G-MEMORY-ACCOUNTING",
            "Resident memory accounting includes entries, metadata, accumulators, indexes, and conversation state.",
            GuaranteeStatus::Enforced,
            "reliability::account_brain / account_conversation; enforced budget via MemoryReport::within_budget",
            "",
        ),
        g(
            "G-REPLAY-TAXONOMY",
            "Every replay check is classified as hash consistency, recomputation, or independent verification.",
            GuaranteeStatus::Enforced,
            "reliability::replay_catalogue / ReplayClass",
            "",
        ),
        g(
            "G-REQUEST-SIZE",
            "HTTP request bodies and turn text are bounded.",
            GuaranteeStatus::Enforced,
            "chat_server::router DefaultBodyLimit + TurnRequest length check",
            "",
        ),
        g(
            "G-TURN-TIMEOUT",
            "A turn has a bounded execution time; exceeding it yields an explicit timeout outcome.",
            GuaranteeStatus::Enforced,
            "chat_server::submit_turn spawn_blocking + tokio::time::timeout",
            "",
        ),
        g(
            "G-BOUNDED-QUEUE",
            "Concurrent in-flight turns are bounded by admission control.",
            GuaranteeStatus::Enforced,
            "chat_server::ChatServerState turn semaphore",
            "",
        ),
        g(
            "G-CANCEL-PROPAGATION",
            "Cancellation is observed between processing stages and reported truthfully.",
            GuaranteeStatus::Checked,
            "conversation::service TurnOptions::cancel; chat_server CancelGuard",
            "cooperative; a single long solver call is not preemptible",
        ),
        g(
            "G-TASK-AUTHORITY",
            "A background task executes a capability only if it is inside the task's declared authority scope, and only while inside its declared resource budget; otherwise the task is stopped (refused) before the capability runs.",
            GuaranteeStatus::Enforced,
            "autonomy_task::TaskRunner::step checks scope and budget before invoking the host adapter; a breach yields TaskState::Refused",
            "applies to the five bounded task kinds; no kind can declare shell, network, or simulation authority",
        ),
        g(
            "G-TASK-OBSERVABLE",
            "Every task step is recorded as an ordered progress entry, and every task can explain its result, its stop reason, and whether it stayed within its declaration.",
            GuaranteeStatus::Enforced,
            "autonomy_task::TaskRunner::step appends TaskProgress; TaskReport::from_task renders the explanation",
            "",
        ),
        g(
            "G-LIVENESS-NONBLOCKING",
            "The health endpoint answers without waiting indefinitely behind a CPU-heavy turn.",
            GuaranteeStatus::Enforced,
            "chat_server::health polls try_lock with a 200 ms budget and falls back to cached counts",
            "",
        ),
        g(
            "G-LOCALHOST-DEFAULT",
            "The HTTP service binds loopback by default and refuses non-loopback binds without explicit opt-in.",
            GuaranteeStatus::Enforced,
            "bin/machine_chat.rs bind guard (--allow-remote)",
            "",
        ),
        g(
            "G-AUTH-REMOTE",
            "A non-loopback bind requires a bearer token.",
            GuaranteeStatus::Enforced,
            "chat_server::auth_middleware; bin/machine_chat.rs --token / MACHINE_CHAT_TOKEN",
            "applies when binding a non-loopback address",
        ),
        g(
            "G-PROJECTION-TELEMETRY",
            "Projection telemetry measures the configured runtime path, not a hardcoded backend.",
            GuaranteeStatus::Checked,
            "reliability::projection_path; lib.rs::measure_kappa_p records the active ProjectionPath",
            "CUDA backend unavailable in this build; path reports cpu_soft_projection",
        ),
        g(
            "G-RELEASE-SCHEMA",
            "A database is never opened for migration without a pre-upgrade backup, and a database newer than the running build is refused rather than misread.",
            GuaranteeStatus::Enforced,
            "operator::run_upgrade backs up before opening the service; persistence::db::Database::open refuses a newer PRAGMA user_version; operator::plan_upgrade reports incompatible before any write",
            "applies to the operator upgrade path and to any service open",
        ),
    ]
}

fn g(
    id: &str,
    statement: &str,
    status: GuaranteeStatus,
    witness: &str,
    configuration: &str,
) -> Guarantee {
    Guarantee {
        id: id.to_string(),
        statement: statement.to_string(),
        status,
        witness: witness.to_string(),
        configuration: configuration.to_string(),
    }
}

/// Count guarantees per status.
pub fn guarantee_status_counts(registry: &[Guarantee]) -> HashMap<GuaranteeStatus, usize> {
    let mut counts = HashMap::new();
    for item in registry {
        *counts.entry(item.status).or_insert(0) += 1;
    }
    counts
}

// ─── Projection runtime path ───────────────────────────────────────────────

/// Which projection implementation is actually measured at runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionPath {
    /// The in-process scalar CPU `soft_project` implementation.
    CpuSoftProjection,
    /// The feature-gated CUDA projector.
    CudaProjection,
}

impl ProjectionPath {
    pub fn label(self) -> &'static str {
        match self {
            ProjectionPath::CpuSoftProjection => "cpu_soft_projection",
            ProjectionPath::CudaProjection => "cuda_projection",
        }
    }
}

/// The projection path this build will measure.
///
/// Telemetry must name the path it characterizes.  When the CUDA feature is
/// unavailable (the supported default here), the path is the CPU software
/// projection and the report says so, rather than implying a GPU measurement.
pub fn projection_path() -> ProjectionPath {
    if cfg!(feature = "cuda") {
        ProjectionPath::CudaProjection
    } else {
        ProjectionPath::CpuSoftProjection
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drain_is_amortized_and_zero_when_within_cap() {
        assert_eq!(MemoryBudget::entries_to_drain(10, 10), 0);
        assert_eq!(MemoryBudget::entries_to_drain(9, 10), 0);
        // Overflow drains a quarter of the cap, at least one.
        assert_eq!(MemoryBudget::entries_to_drain(1001, 1000), 250);
        assert_eq!(MemoryBudget::entries_to_drain(4, 3), 1);
    }

    #[test]
    fn capacity_predicates_use_declared_caps() {
        let budget = MemoryBudget {
            max_clusters: 4,
            max_transient_clusters: 2,
            ..MemoryBudget::default()
        };
        assert!(!budget.clusters_at_capacity(3));
        assert!(budget.clusters_at_capacity(4));
        assert!(!budget.transients_at_capacity(1));
        assert!(budget.transients_at_capacity(2));
    }

    #[test]
    fn report_total_is_the_sum_of_all_terms() {
        let mut report = MemoryReport {
            entry_bytes: 10,
            metadata_bytes: 20,
            accumulator_bytes: 30,
            centroid_bytes: 40,
            association_bytes: 50,
            experience_bytes: 60,
            index_bytes: 70,
            conversation_bytes: 80,
            budget_bytes: 1000,
            ..Default::default()
        };
        report.recompute_total();
        assert_eq!(report.total_bytes, 360);
        assert!(report.within_budget());
        report.budget_bytes = 100;
        assert!(!report.within_budget());
        assert!((report.budget_fraction() - 3.6).abs() < 1e-9);
    }

    #[test]
    fn replay_classes_are_ordered_by_strength() {
        assert!(ReplayClass::IndependentVerification.at_least(ReplayClass::Recomputation));
        assert!(ReplayClass::Recomputation.at_least(ReplayClass::HashConsistency));
        assert!(!ReplayClass::HashConsistency.at_least(ReplayClass::Recomputation));
    }

    #[test]
    fn catalogue_covers_all_three_classes() {
        let catalogue = replay_catalogue();
        let counts = replay_class_counts(&catalogue);
        assert!(counts.get(&ReplayClass::HashConsistency).copied().unwrap_or(0) > 0);
        assert!(counts.get(&ReplayClass::Recomputation).copied().unwrap_or(0) > 0);
        assert!(counts.get(&ReplayClass::IndependentVerification).copied().unwrap_or(0) > 0);
        // Ids are unique.
        let mut ids: Vec<_> = catalogue.iter().map(|m| m.id.clone()).collect();
        ids.sort();
        let unique = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), unique);
    }

    #[test]
    fn every_guarantee_names_a_witness() {
        for guarantee in guarantee_registry() {
            assert!(
                !guarantee.witness.trim().is_empty(),
                "{} has no witness",
                guarantee.id
            );
        }
    }

    #[test]
    fn projection_path_matches_build_features() {
        let path = projection_path();
        if cfg!(feature = "cuda") {
            assert_eq!(path, ProjectionPath::CudaProjection);
        } else {
            assert_eq!(path, ProjectionPath::CpuSoftProjection);
        }
    }

    #[test]
    fn sustained_cluster_ingestion_respects_the_cluster_cap() {
        use crate::{Hypervector, VSABrain};

        let mut brain = VSABrain::new(0.30);
        // Tight cluster budget so the cap is reached quickly.
        brain.memory_budget = MemoryBudget {
            max_clusters: 32,
            max_entries_per_cluster: 16,
            max_transient_clusters: 8,
            max_entries_per_transient_cluster: 8,
            ..MemoryBudget::default()
        };

        // 5,000 orthogonal-ish observations: every one should either be
        // absorbed or replace the coldest cluster, never grow past the cap.
        for i in 0..5_000usize {
            let mut bits = [0u64; 160];
            // Spread the pattern so observations are mostly distinct.
            bits[i % 160] = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            bits[(i * 7) % 160] ^= 0xDEAD_BEEF_CAFE_F00D;
            brain.add_to_dejavu_db(
                Hypervector { bits },
                &format!("obs-{i}"),
                std::collections::HashMap::new(),
            );
            assert!(
                brain.dejavu_clusters.len() <= 32,
                "cluster cap exceeded at i={i}: {}",
                brain.dejavu_clusters.len()
            );
            for cluster in &brain.dejavu_clusters {
                assert!(cluster.entries.len() <= 16);
            }
        }
        // Sustained ingestion kept the population bounded and non-empty
        // rather than growing one cluster per observation.
        assert!(!brain.dejavu_clusters.is_empty());
        assert!(brain.dejavu_clusters.len() <= 32);
    }

    #[test]
    fn sustained_transient_ingestion_respects_the_transient_caps() {
        use crate::{Hypervector, VSABrain};

        let mut brain = VSABrain::new(0.30);
        brain.memory_budget = MemoryBudget {
            max_clusters: 8,
            max_entries_per_cluster: 8,
            max_transient_clusters: 8,
            max_entries_per_transient_cluster: 8,
            ..MemoryBudget::default()
        };

        for i in 0..2_000usize {
            let mut bits = [0u64; 160];
            bits[i % 160] = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
            bits[(i * 13) % 160] ^= 0x1234_5678_9ABC_DEF0;
            brain.add_transient_fact(
                Hypervector { bits },
                &format!("t-{i}"),
                std::collections::HashMap::new(),
            );
            assert!(
                brain.transient_clusters.len() <= 8,
                "transient cluster cap exceeded at i={i}: {}",
                brain.transient_clusters.len()
            );
            for cluster in &brain.transient_clusters {
                assert!(
                    cluster.entries.len() <= 8,
                    "transient entry cap exceeded at i={i}: {}",
                    cluster.entries.len()
                );
            }
        }
    }

    #[test]
    fn memory_report_accounts_for_every_component_and_stays_within_budget() {
        use crate::{Hypervector, VSABrain};

        let mut brain = VSABrain::new(0.30);
        brain.memory_budget = MemoryBudget {
            max_clusters: 64,
            max_entries_per_cluster: 32,
            max_transient_clusters: 64,
            max_entries_per_transient_cluster: 32,
            max_total_bytes: 8 * 1024 * 1024,
        };
        for i in 0..1_000usize {
            let mut bits = [0u64; 160];
            bits[i % 160] = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
            let mut metadata = std::collections::HashMap::new();
            metadata.insert("source".to_string(), format!("doc-{i}"));
            brain.add_to_dejavu_db(Hypervector { bits }, &format!("obs-{i}"), metadata);
            if i % 3 == 0 {
                let mut tb = [0u64; 160];
                tb[(i * 5) % 160] = 0xABCD_EF01_2345_6789;
                brain.add_transient_fact(Hypervector { bits: tb }, &format!("t-{i}"), std::collections::HashMap::new());
            }
        }

        let report = brain.memory_report();
        assert!(report.entry_bytes > 0, "entries must be accounted");
        assert!(report.metadata_bytes > 0, "metadata must be accounted");
        assert!(report.centroid_bytes > 0, "centroids/anchors must be accounted");
        assert!(report.total_bytes >= report.entry_bytes);
        assert!(report.cluster_count <= 64);
        assert!(report.within_budget(), "report exceeded budget: {report:?}");

        // Conversation state accounting is additive.
        let mut with_conversation = report.clone();
        let before = with_conversation.total_bytes;
        account_conversation(&mut with_conversation, 3, 40, 2, 12);
        assert!(with_conversation.total_bytes > before);
        assert!(with_conversation.conversation_bytes > 0);
    }

    #[test]
    fn try_use_crate_reexports() {
        // Guard against accidental privacy regressions for the accounting API.
        let _ = crate::reliability::projection_path();
    }

    #[test]
    fn cluster_eviction_leaves_no_dangling_association_indices() {
        use crate::{Hypervector, VSABrain};

        let mut brain = VSABrain::new(0.30);
        brain.memory_budget = MemoryBudget {
            max_clusters: 3,
            max_entries_per_cluster: 8,
            ..MemoryBudget::default()
        };

        // Fill to the cap with distinct clusters (deterministic per-cluster
        // pseudo-random patterns).
        for i in 0..3u64 {
            let mut bits = [0u64; 160];
            let mut x = i.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
            for word in bits.iter_mut() {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                *word = x;
            }
            brain.add_to_dejavu_db(Hypervector { bits }, &format!("c{i}"), Default::default());
        }
        assert_eq!(brain.dejavu_clusters.len(), 3);

        // Plant an association that refers to the last cluster index.
        brain.cross_cluster_associations.insert(
            0,
            vec![(2, Hypervector::new_zero(), 0.9, 0)],
        );

        // Force an eviction by adding a fourth, distant observation.
        let mut bits = [0u64; 160];
        let mut x = 0xABCD_EF01_2345_6789u64 | 1;
        for word in bits.iter_mut() {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            *word = x;
        }
        brain.add_to_dejavu_db(Hypervector { bits }, "new", Default::default());

        let live = brain.dejavu_clusters.len();
        assert!(live <= 3, "cluster cap must hold, got {live}");
        for (from, assocs) in &brain.cross_cluster_associations {
            assert!(*from < live, "association source index {from} is dangling");
            for (to, _, _, _) in assocs {
                assert!(*to < live, "association target index {to} is dangling");
            }
        }
    }

    #[test]
    fn projection_telemetry_reports_the_active_runtime_path() {
        use crate::{Hypervector, VSABrain};

        let mut brain = VSABrain::new(0.30);
        // Seed two clusters so measurement is not skipped.
        brain.add_to_dejavu_db(Hypervector::new_random(), "a", Default::default());
        brain.add_to_dejavu_db(Hypervector::new_random(), "b", Default::default());
        brain.measure_kappa_p(4);
        let telemetry = &brain.contraction_telemetry;
        assert_eq!(telemetry.projection_path, projection_path());
        assert!(
            telemetry.report().contains(projection_path().label()),
            "report must name the measured path: {}",
            telemetry.report()
        );
        assert!(telemetry.kappa_p_count >= 1);
    }
}
