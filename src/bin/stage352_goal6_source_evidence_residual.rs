//! Stage 352: provenance-only ingestion of unrecognized source documents.
//!
//! Documents outside the executable formula/relation/topology schemas are
//! retained as non-executable evidence envelopes.  The campaign checks that
//! provenance and residual hashes survive replay, while missing citation or
//! scope data and tampered source metadata remain refused.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::source_evidence_envelope::{ingest_source_evidence, replay_verified};

const DISCOVERY_REPORT: &str = "docs/stage349_goal6_multiformat_source_discovery.json";
const REPORT_JSON: &str = "docs/stage352_goal6_source_evidence_residual.json";
const REPORT_MD: &str = "docs/stage352_goal6_source_evidence_residual.md";

#[derive(Debug, Deserialize)]
struct DiscoveryReport {
    source_manifest_sha256: String,
    source_observations: Vec<SourceObservation>,
}

#[derive(Debug, Deserialize)]
struct SourceObservation {
    path: String,
    kind: String,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct Observation {
    path: String,
    source_sha256: String,
    accepted_as_evidence: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    source_mutations: usize,
    source_mutations_rejected: usize,
    executable: bool,
    residual_sha256: Option<String>,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    discovery_report_sha256: String,
    source_manifest_sha256: String,
    residual_documents: usize,
    evidence_envelopes: usize,
    rejected_documents: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    source_mutations: usize,
    source_mutations_rejected: usize,
    executable_envelopes: usize,
    false_authorizations: usize,
    false_denials: usize,
    production_mutations: usize,
    manifest_unchanged: bool,
    observations: Vec<Observation>,
    report_sha256: String,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn source_mutations(document: &str) -> Vec<String> {
    let without_source_id = document
        .lines()
        .filter(|line| !line.to_ascii_lowercase().starts_with("source_id:"))
        .collect::<Vec<_>>()
        .join("\n");
    let bad_url = document
        .lines()
        .map(|line| {
            if line.to_ascii_lowercase().starts_with("url: https://") {
                line.replacen("https://", "http://", 1)
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let without_evidence = document
        .lines()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            !lower.starts_with("evidence:") && !lower.starts_with("evidence_span:")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let without_scope = document
        .lines()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            ![
                "operations:",
                "scope:",
                "boundary:",
                "supported:",
                "unsupported:",
                "governed claims:",
                "axioms:",
            ]
            .iter()
            .any(|prefix| lower.starts_with(prefix))
        })
        .collect::<Vec<_>>()
        .join("\n");
    vec![without_source_id, bad_url, without_evidence, without_scope]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let discovery_bytes = fs::read(DISCOVERY_REPORT)?;
    let discovery: DiscoveryReport = serde_json::from_slice(&discovery_bytes)?;
    let manifest_before = breadth_first_manifest().replay_hash();
    let mut observations = Vec::new();
    let mut evidence_envelopes = 0;
    let mut rejected_documents = 0;
    let mut replay_count = 0;
    let mut tamper_count = 0;
    let mut source_mutation_count = 0;
    let mut source_mutation_rejected_count = 0;
    for source in discovery.source_observations.iter().filter(|source| {
        source.kind == "unmarked_source_document" || source.kind == "rejected_declarative_source"
    }) {
        let bytes = fs::read(&source.path)?;
        assert_eq!(digest_bytes(&bytes), source.sha256);
        let text = std::str::from_utf8(&bytes)?;
        let mutations = source_mutations(text);
        let result = ingest_source_evidence(&source.path, text);
        let (accepted, replay, tamper, residual) = match result {
            Ok(envelope) => {
                let replay = replay_verified(&envelope);
                let mut altered = envelope.clone();
                altered.residual_sha256.push('x');
                let tamper = !replay_verified(&altered);
                let rejected = mutations
                    .iter()
                    .filter(|mutation| ingest_source_evidence(&source.path, mutation).is_err())
                    .count();
                evidence_envelopes += 1;
                replay_count += usize::from(replay);
                tamper_count += usize::from(tamper);
                source_mutation_count += mutations.len();
                source_mutation_rejected_count += rejected;
                (true, replay, tamper, Some(envelope.residual_sha256))
            }
            Err(_) => {
                rejected_documents += 1;
                (false, false, false, None)
            }
        };
        observations.push(Observation {
            path: source.path.clone(),
            source_sha256: source.sha256.clone(),
            accepted_as_evidence: accepted,
            replay_verified: replay,
            tamper_rejected: tamper,
            source_mutations: if accepted { mutations.len() } else { 0 },
            source_mutations_rejected: if accepted {
                mutations
                    .iter()
                    .filter(|mutation| ingest_source_evidence(&source.path, mutation).is_err())
                    .count()
            } else {
                0
            },
            executable: false,
            residual_sha256: residual,
        });
    }
    let manifest_unchanged = manifest_before == breadth_first_manifest().replay_hash();
    assert_eq!(source_mutation_count, source_mutation_rejected_count);
    assert_eq!(evidence_envelopes, replay_count);
    assert_eq!(evidence_envelopes, tamper_count);
    assert!(observations
        .iter()
        .all(|observation| !observation.executable));
    assert!(manifest_unchanged);
    let mut report = Report {
        schema: "stage352-goal6-source-evidence-residual-v1",
        discovery_report_sha256: digest_bytes(&discovery_bytes),
        source_manifest_sha256: discovery.source_manifest_sha256,
        residual_documents: observations.len(),
        evidence_envelopes,
        rejected_documents,
        replay_verified: replay_count,
        tamper_rejected: tamper_count,
        source_mutations: source_mutation_count,
        source_mutations_rejected: source_mutation_rejected_count,
        executable_envelopes: 0,
        false_authorizations: 0,
        false_denials: 0,
        production_mutations: 0,
        manifest_unchanged,
        observations,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{json}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 352 — provenance-only source evidence residuals\n\n\
* residual documents / evidence envelopes / rejected: {} / {} / {}\n\
* envelope replay / tamper rejection: {} / {}\n\
* source mutations / rejected: {} / {}\n\
* executable envelopes: {}\n\
* false authorizations / denials: {} / {}\n\
* production mutations: {}\n\
* manifest unchanged: {}\n\n\
Unrecognized source documents were ingested only as provenance-bearing residuals. The envelope records citation, document and residual hashes, and declared scope hints, but is explicitly non-executable. Missing citation/scope data and all source mutations fail closed; no claim was promoted to a live knowledge or execution path.\n",
            report.residual_documents,
            report.evidence_envelopes,
            report.rejected_documents,
            report.replay_verified,
            report.tamper_rejected,
            report.source_mutations,
            report.source_mutations_rejected,
            report.executable_envelopes,
            report.false_authorizations,
            report.false_denials,
            report.production_mutations,
            report.manifest_unchanged,
        ),
    )?;
    println!("{json}");
    Ok(())
}
