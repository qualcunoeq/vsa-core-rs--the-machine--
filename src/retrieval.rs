// ─── Cross-Domain Retrieval ─────────────────────────────────────────────────
//
// The join that connects all encoders.  Cross-domain retrieval answers
// questions that no single encoder could answer alone:
//
//   "What is supposed to be on port 8000?"
//     → text_knowledge:  (backend, listens_on, port_8000)
//     → system_state:    (process_curl, connected_to, port_8000)
//     → ANOMALY: unexpected process on expected port
//
// This is what Harold Finch's Machine does — it knows what normal looks
// like from documentation, compares it to what's actually running, and
// reasons about the difference.
// ────────────────────────────────────────────────────────────────────────────

use std::collections::{HashMap, HashSet};
use crate::VSABrain;
use crate::perception::SvoTriple;

/// A query across one or more domains.
#[derive(Debug, Clone)]
pub struct CrossDomainQuery {
    /// Entity to search for (e.g., "port_8000", "process_curl").
    pub entity: String,
    /// Optional relation filter (e.g., "listens_on", None = any).
    pub relation: Option<String>,
    /// Optional object filter (e.g., "interest rates", None = any).
    pub object: Option<String>,
    /// Which domains to search (e.g., ["text_knowledge", "system_state"]).
    pub domains: Vec<String>,
}

/// A single retrieval result.
#[derive(Debug, Clone)]
pub struct CrossDomainResult {
    /// Domain this result came from.
    pub domain: String,
    /// The stored SVO triple.
    pub triple: SvoTriple,
    /// Confidence score (0.0-1.0) based on NLP extraction confidence + cluster density.
    pub confidence: f64,
    /// Index of the cluster that contained this result.
    pub cluster_idx: usize,
}

/// DomainIndex maps cluster indices to their domain labels.
///
/// Built by scanning all cluster entries and extracting the "domain" metadata
/// field.  Supports filtering by domain for cross-domain retrieval.
pub struct DomainIndex {
    /// Cluster index → domain label.
    cluster_to_domain: HashMap<usize, String>,
    /// Domain label → set of cluster indices.
    domain_to_clusters: HashMap<String, Vec<usize>>,
}

impl DomainIndex {
    pub fn new() -> Self {
        DomainIndex {
            cluster_to_domain: HashMap::new(),
            domain_to_clusters: HashMap::new(),
        }
    }

    /// Build the index by scanning all clusters and their entries for domain metadata.
    pub fn build_from_brain(&mut self, brain: &VSABrain) {
        self.cluster_to_domain.clear();
        self.domain_to_clusters.clear();

        for (idx, cluster) in brain.dejavu_clusters.iter().enumerate() {
            // Determine domain from the most common domain tag among entries
            let mut domain_counts: HashMap<String, usize> = HashMap::new();

            for entry in &cluster.entries {
                let domain = entry.metadata.get("domain")
                    .or_else(|| entry.metadata.get("source"))
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string());
                *domain_counts.entry(domain).or_insert(0) += 1;
            }

            if let Some((domain, _)) = domain_counts.into_iter().max_by_key(|&(_, count)| count) {
                self.cluster_to_domain.insert(idx, domain.clone());
                self.domain_to_clusters.entry(domain).or_insert_with(Vec::new).push(idx);
            }
        }
    }

    /// Get all known domain labels.
    pub fn domains(&self) -> Vec<String> {
        let mut domains: Vec<String> = self.domain_to_clusters.keys().cloned().collect();
        domains.sort();
        domains
    }

    /// Get cluster indices for a given domain.
    pub fn clusters_for_domain(&self, domain: &str) -> Vec<usize> {
        self.domain_to_clusters.get(domain).cloned().unwrap_or_default()
    }

    /// Get the domain label for a cluster index.
    pub fn domain_for_cluster(&self, cluster_idx: usize) -> Option<&str> {
        self.cluster_to_domain.get(&cluster_idx).map(|s| s.as_str())
    }
}

/// Retrieve triples across domains matching a query.
///
/// Scans clusters filtered by domain, then checks each entry's stored
/// subject/verb/object metadata against the query.  Returns results
/// sorted by confidence descending.
pub fn retrieve_cross_domain(
    query: &CrossDomainQuery,
    brain: &VSABrain,
    index: &DomainIndex,
) -> Vec<CrossDomainResult> {
    let mut results = Vec::new();

    // Collect cluster indices for requested domains
    let mut cluster_indices: HashSet<usize> = HashSet::new();
    for domain in &query.domains {
        for idx in index.clusters_for_domain(domain) {
            cluster_indices.insert(idx);
        }
    }

    // Search each matching cluster
    for &cluster_idx in &cluster_indices {
        if cluster_idx >= brain.dejavu_clusters.len() {
            continue;
        }
        let cluster = &brain.dejavu_clusters[cluster_idx];
        let domain = index.domain_for_cluster(cluster_idx).unwrap_or("unknown");

        for entry in &cluster.entries {
            let subject = entry.metadata.get("subject");
            let verb = entry.metadata.get("verb");
            let obj = entry.metadata.get("object");

            // Match query entity against subject, verb, or object
            let entity_match = subject.map_or(false, |s| s.contains(&query.entity))
                || verb.map_or(false, |v| v.contains(&query.entity))
                || obj.map_or(false, |o| o.contains(&query.entity));

            if !entity_match {
                continue;
            }

            // Optional relation filter
            if let Some(ref rel) = query.relation {
                if verb.map_or(true, |v| v != rel) {
                    continue;
                }
            }

            // Optional object filter
            if let Some(ref obj_q) = query.object {
                if obj.map_or(true, |o| !o.contains(obj_q)) {
                    continue;
                }
            }

            // Compute confidence from entry label (NLP extraction confidence)
            let confidence: f64 = entry.label.parse().unwrap_or(0.5);

            // Cluster density bonus: entries in denser clusters are more reliable
            let density_bonus = (cluster.entries.len() as f64)
                / (crate::MAX_ENTRIES_PER_CLUSTER as f64).max(1.0);

            let adjusted_conf = (confidence * 0.7 + density_bonus * 0.3).clamp(0.0, 1.0);

            results.push(CrossDomainResult {
                domain: domain.to_string(),
                triple: (
                    subject.cloned().unwrap_or_default(),
                    verb.cloned().unwrap_or_default(),
                    obj.cloned().unwrap_or_default(),
                ),
                confidence: adjusted_conf,
                cluster_idx,
            });
        }
    }

    // Sort by confidence descending
    results.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
    results
}

/// Detect anomalies by comparing what documentation says should be happening
/// (text_knowledge) with what the system is actually doing (system_state).
///
/// Returns ThreatEvents for entities where the expected and actual don't match.
pub fn detect_cross_domain_anomalies(
    brain: &VSABrain,
    index: &DomainIndex,
) -> Vec<crate::defense::ThreatEvent> {
    use crate::defense::{ThreatEvent, ThreatClass};

    let mut events = Vec::new();

    // Get all system_state connections
    let sys_query = CrossDomainQuery {
        entity: String::new(),       // match everything
        relation: Some("connected_to".to_string()),
        object: None,
        domains: vec!["system_state".to_string()],
    };
    let sys_results = retrieve_cross_domain(&sys_query, brain, index);

    // Group by connected entity (object)
    let mut conns_by_target: HashMap<String, Vec<String>> = HashMap::new();
    for r in &sys_results {
        let target = r.triple.2.clone();
        let process = r.triple.0.clone();
        conns_by_target.entry(target).or_insert_with(Vec::new).push(process);
    }

    // Get all text_knowledge about listening services
    let txt_query = CrossDomainQuery {
        entity: String::new(),
        relation: Some("listens_on".to_string()),
        object: None,
        domains: vec!["text_knowledge".to_string()],
    };
    let txt_results = retrieve_cross_domain(&txt_query, brain, index);

    // For each text knowledge service, check if the system has a matching process
    for tr in &txt_results {
        let expected_service = tr.triple.0.clone();    // "backend"
        let port = tr.triple.2.clone();                // "port_8000"

        // Check if the expected service is listed as a process in system state
        let matching_process = sys_results.iter().any(|sr| {
            sr.triple.0.contains(&expected_service)
        });

        if !matching_process {
            // The expected service doesn't seem to be running
            // Check if something else is on that port
            if let Some(actual_processes) = conns_by_target.get(&port) {
                for p in actual_processes {
                    if !p.contains(&expected_service) {
                        events.push(ThreatEvent {
                            domain: "cross_domain".to_string(),
                            severity: 0.7,
                            entity: port.clone(),
                            description: format!(
                                "Mismatch: documentation says '{}' should be on {}, \
                                 but actual system shows '{}' connected",
                                expected_service, port, p
                            ),
                            class: ThreatClass::Threat,
                        });
                    }
                }
            }
        }
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VSABrain;
    use crate::text_encoder::store_knowledge_triple;

    fn setup_brain_with_text_and_system() -> (VSABrain, DomainIndex) {
        let mut brain = VSABrain::new(0.12);

        // Text knowledge: what should be running
        store_knowledge_triple(&mut brain, "backend", "listens_on", "port_8000", 0.9, "text_knowledge");
        store_knowledge_triple(&mut brain, "database", "listens_on", "port_5432", 0.9, "text_knowledge");

        // System state: what is actually running (normal case)
        store_knowledge_triple(&mut brain, "backend", "connected_to", "port_8000", 0.95, "system_state");
        store_knowledge_triple(&mut brain, "database", "connected_to", "port_5432", 0.95, "system_state");

        // Build domain index
        let mut index = DomainIndex::new();
        index.build_from_brain(&brain);
        (brain, index)
    }

    #[test]
    fn test_domain_index_builds() {
        let (_, index) = setup_brain_with_text_and_system();
        let domains = index.domains();
        eprintln!("  Domains found: {:?}", domains);
        assert!(domains.contains(&"text_knowledge".to_string()));
        assert!(domains.contains(&"system_state".to_string()));
    }

    #[test]
    fn test_retrieve_by_entity() {
        let (brain, index) = setup_brain_with_text_and_system();

        let query = CrossDomainQuery {
            entity: "port_8000".to_string(),
            relation: None,
            object: None,
            domains: vec!["text_knowledge".to_string(), "system_state".to_string()],
        };

        let results = retrieve_cross_domain(&query, &brain, &index);
        eprintln!("  Results for 'port_8000': {} found", results.len());
        for r in &results {
            eprintln!("    [{}] ({}, {}, {}) conf={:.3}",
                r.domain, r.triple.0, r.triple.1, r.triple.2, r.confidence);
        }
        assert_eq!(results.len(), 2, "Should find port_8000 in both domains");
    }

    #[test]
    fn test_retrieve_by_domain_filter() {
        let (brain, index) = setup_brain_with_text_and_system();

        let query = CrossDomainQuery {
            entity: "port_8000".to_string(),
            relation: None,
            object: None,
            domains: vec!["text_knowledge".to_string()],  // only text
        };

        let results = retrieve_cross_domain(&query, &brain, &index);
        assert_eq!(results.len(), 1, "Should find port_8000 only in text_knowledge");
        assert_eq!(results[0].domain, "text_knowledge");
    }

    #[test]
    fn test_retrieve_by_relation() {
        let (brain, index) = setup_brain_with_text_and_system();

        let query = CrossDomainQuery {
            entity: "backend".to_string(),
            relation: Some("listens_on".to_string()),
            object: None,
            domains: vec!["text_knowledge".to_string()],
        };

        let results = retrieve_cross_domain(&query, &brain, &index);
        assert!(!results.is_empty(), "Should find backend listens_on");
        assert_eq!(results[0].triple.1, "listens_on");
    }

    #[test]
    fn test_anomaly_detection() {
        let mut brain = VSABrain::new(0.12);

        // Text knowledge: what should be running
        store_knowledge_triple(&mut brain, "backend", "listens_on", "port_8000", 0.9, "text_knowledge");

        // System state: something unexpected on port 8000
        store_knowledge_triple(&mut brain, "process_curl", "connected_to", "port_8000", 0.95, "system_state");

        let mut index = DomainIndex::new();
        index.build_from_brain(&brain);

        let events = detect_cross_domain_anomalies(&brain, &index);
        eprintln!("  Anomaly events: {} found", events.len());
        for e in &events {
            eprintln!("    [{}] severity={:.1}: {}", e.domain, e.severity, e.description);
        }

        assert!(!events.is_empty(), "Should detect anomaly: curl on backend's port");
        let has_anomaly = events.iter().any(|e| e.description.contains("Mismatch"));
        assert!(has_anomaly, "Should report mismatch between expected and actual");
    }

    #[test]
    fn test_no_false_positive_when_matching() {
        let mut brain = VSABrain::new(0.12);

        // Text knowledge: backend on port 8000
        store_knowledge_triple(&mut brain, "backend", "listens_on", "port_8000", 0.9, "text_knowledge");

        // System state: actual backend is indeed on port 8000
        store_knowledge_triple(&mut brain, "backend", "connected_to", "port_8000", 0.95, "system_state");

        let mut index = DomainIndex::new();
        index.build_from_brain(&brain);

        let events = detect_cross_domain_anomalies(&brain, &index);
        eprintln!("  Events when matching: {} found", events.len());
        assert!(events.is_empty(), "No anomaly when backend actually runs on port 8000");
    }
}
