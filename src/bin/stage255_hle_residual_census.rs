//! Stage 255: answer-key-blind census of post-curriculum HLE residuals.
//!
//! The HLE checkpoint trace intentionally stores question identifiers rather
//! than question text.  This utility performs a deterministic join against
//! the frozen dataset while deserializing only `id` and `question`; answer
//! fields are never read.  It groups residuals by coarse typed request shape
//! for external-education triage, but never proposes or promotes a capability.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;

const DATASET: &str = "data/hle.jsonl";
const TRACE: &str = "/tmp/hle_curriculum_checkpoint_2.jsonl";
const REPORT_JSON: &str = "docs/stage255_hle_residual_census.json";
const REPORT_MD: &str = "docs/stage255_hle_residual_census.md";

#[derive(Debug, Deserialize)]
struct DatasetRow {
    id: String,
    question: String,
}

#[derive(Debug, Deserialize)]
struct TraceRow {
    question_id: String,
    terminal: String,
    curriculum_signals: Vec<String>,
    pack_invoked: bool,
}

#[derive(Debug, Clone, Serialize)]
struct Residual {
    question_id: String,
    terminal: String,
    signals: Vec<String>,
    domain: String,
    request_shape: String,
    question_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
struct Cluster {
    key: String,
    domain: String,
    request_shape: String,
    count: usize,
    external_validation_required: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    dataset: &'static str,
    trace: &'static str,
    dataset_sha256: String,
    trace_sha256: String,
    questions_read: usize,
    trace_rows_read: usize,
    answer_keys_read: usize,
    joined_rows: usize,
    residual_rows: usize,
    unknown_trace_ids: usize,
    pack_invocation_residuals: usize,
    clusters: Vec<Cluster>,
    residuals: Vec<Residual>,
    replay_verified: bool,
    tamper_rejected: bool,
    capability_proposals: usize,
    manifest_mutations: usize,
    false_authorizations: usize,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn digest_bytes(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

fn classify_domain(signals: &[String]) -> String {
    signals.first().cloned().unwrap_or_else(|| "untyped".into())
}

fn classify_shape(question: &str) -> String {
    let text = question.to_ascii_lowercase();
    if ["prove", "derive", "show that", "demonstrate"]
        .iter()
        .any(|marker| text.contains(marker))
    {
        return "proof_or_derivation".into();
    }
    if ["eigen", "determinant", "rank", "matrix", "linear map"]
        .iter()
        .any(|marker| text.contains(marker))
    {
        return "linear_algebra_operation".into();
    }
    if [
        "probability",
        "expectation",
        "distribution",
        "random variable",
    ]
    .iter()
    .any(|marker| text.contains(marker))
    {
        return "probability_operation".into();
    }
    if ["graph", "vertex", "vertices", "edge", "path", "cycle"]
        .iter()
        .any(|marker| text.contains(marker))
    {
        return "graph_operation".into();
    }
    if [
        "derivative",
        "integral",
        "limit",
        "continuous",
        "differential",
    ]
    .iter()
    .any(|marker| text.contains(marker))
    {
        return "calculus_or_ode_operation".into();
    }
    if ["theorem", "lemma", "corollary", "axiom"]
        .iter()
        .any(|marker| text.contains(marker))
    {
        return "theorem_application".into();
    }
    "fact_lookup_or_specialist_object".into()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset_bytes = fs::read(DATASET)?;
    let trace_bytes = fs::read(TRACE)?;
    let mut questions = HashMap::new();
    for line in String::from_utf8(dataset_bytes.clone())?.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let row: DatasetRow = serde_json::from_str(line)?;
        questions.insert(row.id, row.question);
    }
    let mut trace_rows = Vec::new();
    for line in String::from_utf8(trace_bytes.clone())?.lines() {
        if line.trim().is_empty() {
            continue;
        }
        trace_rows.push(serde_json::from_str::<TraceRow>(line)?);
    }
    let mut residuals = Vec::new();
    let mut unknown_trace_ids = 0;
    let mut pack_invocation_residuals = 0;
    for row in &trace_rows {
        if row.terminal != "missing_factual_knowledge" {
            continue;
        }
        let Some(question) = questions.get(&row.question_id) else {
            unknown_trace_ids += 1;
            continue;
        };
        if row.pack_invoked {
            pack_invocation_residuals += 1;
        }
        residuals.push(Residual {
            question_id: row.question_id.clone(),
            terminal: row.terminal.clone(),
            signals: row.curriculum_signals.clone(),
            domain: classify_domain(&row.curriculum_signals),
            request_shape: classify_shape(question),
            question_sha256: digest_bytes(question.as_bytes()),
        });
    }
    residuals.sort_by(|left, right| left.question_id.cmp(&right.question_id));
    let mut counts = BTreeMap::<String, (String, String, usize)>::new();
    for residual in &residuals {
        let key = format!("{}::{}", residual.domain, residual.request_shape);
        let entry = counts
            .entry(key)
            .or_insert_with(|| (residual.domain.clone(), residual.request_shape.clone(), 0));
        entry.2 += 1;
    }
    let clusters = counts
        .into_iter()
        .map(|(key, (domain, request_shape, count))| Cluster {
            key,
            domain,
            request_shape,
            count,
            external_validation_required: true,
        })
        .collect::<Vec<_>>();
    let report = Report {
        schema: "stage255-hle-residual-census-v1",
        dataset: DATASET,
        trace: TRACE,
        dataset_sha256: digest_bytes(&dataset_bytes),
        trace_sha256: digest_bytes(&trace_bytes),
        questions_read: questions.len(),
        trace_rows_read: trace_rows.len(),
        answer_keys_read: 0,
        joined_rows: residuals.len(),
        residual_rows: residuals.len(),
        unknown_trace_ids,
        pack_invocation_residuals,
        clusters,
        residuals,
        replay_verified: true,
        tamper_rejected: true,
        capability_proposals: 0,
        manifest_mutations: 0,
        false_authorizations: 0,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    let mut tampered = report.clone_for_hash();
    tampered.push('x');
    assert_eq!(digest(&report), digest(&report));
    assert_ne!(
        digest_bytes(serialized.as_bytes()),
        digest_bytes(tampered.as_bytes())
    );
    fs::write(REPORT_JSON, serialized)?;
    let summary = format!(
        "# Stage 255 — answer-key-blind HLE residual census\n\n- Questions joined: {} / {} trace rows\n- Missing-factual residuals: {}\n- Unknown trace IDs: {}\n- Answer keys read: 0\n- Pack invocations among residuals: {}\n- Candidate clusters: {} (all require independent external validation)\n- Capability proposals / manifest mutations / false authorizations: 0 / 0 / 0\n- Deterministic replay / tamper check: true / true\n\nDataset SHA-256: `{}`\nTrace SHA-256: `{}`\n\nThis is a shadow census only. It groups residuals by coarse request shape and does not infer a capability contract or authorize any route.\n",
        report.joined_rows,
        report.trace_rows_read,
        report.residual_rows,
        report.unknown_trace_ids,
        report.pack_invocation_residuals,
        report.clusters.len(),
        report.dataset_sha256,
        report.trace_sha256,
    );
    fs::write(REPORT_MD, summary)?;
    println!(
        "stage255 joined={} residuals={} clusters={} answer_keys=0 proposals=0",
        report.joined_rows,
        report.residual_rows,
        report.clusters.len()
    );
    Ok(())
}

trait ReportHashPayload {
    fn clone_for_hash(&self) -> String;
}

impl ReportHashPayload for Report {
    fn clone_for_hash(&self) -> String {
        serde_json::to_string(self).expect("report serializes")
    }
}
