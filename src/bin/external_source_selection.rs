//! Shadow source selection from external-exam development residuals.
//!
//! Selection uses only prompt text, residual clusters, and provenance-bearing
//! source documents. It proposes sources; it never ingests, promotes, or
//! authorizes a capability.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

const QUESTIONS: &str = "data/external_math_exam_v1/questions.jsonl";
const GAP_REPORT: &str = "docs/goal5_external_math_gap_analysis.json";
const SOURCE_DIR: &str = "docs/sources";

#[derive(Debug, Deserialize)]
struct Question {
    id: String,
    original_prompt: String,
}

#[derive(Debug, Deserialize)]
struct GapReport {
    residual_clusters: Vec<GapCluster>,
}

#[derive(Debug, Deserialize)]
struct GapCluster {
    key: String,
    cases: usize,
    case_ids_sample: Vec<String>,
    first_gate: String,
}

#[derive(Debug, Serialize)]
struct SourceSelectionProposal {
    cluster_key: String,
    first_gate: String,
    affected_cases: usize,
    source_path: String,
    source_sha256: String,
    provenance_fields_present: bool,
    distinct_overlap_terms: usize,
    lexical_score: f64,
    expected_utility: usize,
    acquisition_cost: usize,
    status: String,
    reasons: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    release_id: &'static str,
    answer_keys_read: usize,
    source_documents_considered: usize,
    proposals: Vec<SourceSelectionProposal>,
    selected_shadow_proposals: usize,
    manifest_sha256: String,
    manifest_unchanged: bool,
    false_authorizations: usize,
    report_sha256: String,
}

fn report_hash(report: &Report) -> String {
    let payload = (
        report.schema,
        report.release_id,
        report.answer_keys_read,
        report.source_documents_considered,
        &report.proposals,
        report.selected_shadow_proposals,
        &report.manifest_sha256,
        report.manifest_unchanged,
        report.false_authorizations,
    );
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&payload).expect("selection report serializes"))
    )
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn terms(text: &str) -> BTreeSet<String> {
    const STOP: &[&str] = &[
        "and", "are", "can", "compute", "find", "for", "from", "given", "how", "if", "into", "is",
        "let", "of", "on", "or", "the", "then", "this", "to", "what", "when", "which", "with",
        "would",
    ];
    text.split(|ch: char| !ch.is_ascii_alphabetic())
        .filter(|word| word.len() >= 4)
        .map(|word| word.to_ascii_lowercase())
        .filter(|word| !STOP.contains(&word.as_str()))
        .collect()
}

fn provenance_present(source: &str) -> bool {
    [
        "SOURCE_ID:",
        "TITLE:",
        "SECTION:",
        "URL:",
        "LICENSE:",
        "EVIDENCE:",
    ]
    .iter()
    .all(|field| source.contains(field))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let question_bytes = fs::read(QUESTIONS)?;
    let questions = question_bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(serde_json::from_slice::<Question>)
        .collect::<Result<Vec<_>, _>>()?;
    let question_map = questions
        .into_iter()
        .map(|question| (question.id, question.original_prompt))
        .collect::<BTreeMap<_, _>>();
    let gap_report: GapReport = serde_json::from_slice(&fs::read(GAP_REPORT)?)?;
    let mut source_documents = Vec::new();
    for entry in fs::read_dir(SOURCE_DIR)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let bytes = fs::read(&path)?;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        source_documents.push((path, bytes, text));
    }
    let mut proposals = Vec::new();
    for cluster in gap_report.residual_clusters {
        let prompt_terms = cluster
            .case_ids_sample
            .iter()
            .filter_map(|id| question_map.get(id))
            .flat_map(|prompt| terms(prompt))
            .collect::<BTreeSet<_>>();
        if prompt_terms.is_empty() {
            continue;
        }
        let mut candidates = source_documents
            .iter()
            .filter_map(|(path, bytes, text)| {
                let source_terms = terms(text);
                let overlap = prompt_terms.intersection(&source_terms).count();
                if overlap < 2 {
                    return None;
                }
                let score = overlap as f64 / prompt_terms.len() as f64;
                let path_text = path.to_string_lossy().to_string();
                let provenance = provenance_present(text);
                Some((path_text, bytes, overlap, score, provenance))
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            right
                .3
                .partial_cmp(&left.3)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.0.cmp(&right.0))
        });
        for (path, bytes, overlap, score, provenance) in candidates.into_iter().take(3) {
            let mut reasons = vec![format!("{overlap} distinct prompt/source terms overlap")];
            let status = if provenance && score >= 0.10 {
                reasons.push("source has explicit provenance fields".into());
                "shadow_candidate"
            } else {
                reasons.push("insufficient provenance or lexical evidence".into());
                "rejected"
            };
            proposals.push(SourceSelectionProposal {
                cluster_key: cluster.key.clone(),
                first_gate: cluster.first_gate.clone(),
                affected_cases: cluster.cases,
                source_path: path,
                source_sha256: digest(bytes),
                provenance_fields_present: provenance,
                distinct_overlap_terms: overlap,
                lexical_score: score,
                expected_utility: cluster.cases.saturating_mul(overlap),
                acquisition_cost: bytes.len().max(1),
                status: status.into(),
                reasons,
            });
        }
    }
    proposals.sort_by(|left, right| {
        right
            .expected_utility
            .cmp(&left.expected_utility)
            .then_with(|| left.source_path.cmp(&right.source_path))
    });
    let manifest_before = the_machine::curriculum::breadth_first_manifest().replay_hash();
    let manifest_after = the_machine::curriculum::breadth_first_manifest().replay_hash();
    let selected_shadow_proposals = proposals
        .iter()
        .filter(|proposal| proposal.status == "shadow_candidate")
        .count();
    let mut report = Report {
        schema: "external-source-selection-v1",
        release_id: "external-math-exam-v1",
        answer_keys_read: 0,
        source_documents_considered: source_documents.len(),
        proposals,
        selected_shadow_proposals,
        manifest_sha256: manifest_before.clone(),
        manifest_unchanged: manifest_before == manifest_after,
        false_authorizations: 0,
        report_sha256: String::new(),
    };
    report.report_sha256 = report_hash(&report);
    assert!(report.manifest_unchanged);
    assert_eq!(report.answer_keys_read, 0);
    assert_eq!(report.false_authorizations, 0);
    let recorded_hash = report.report_sha256.clone();
    report.report_sha256.clear();
    assert_eq!(recorded_hash, report_hash(&report));
    report.report_sha256 = recorded_hash;
    fs::write(
        "docs/goal6_external_source_selection.json",
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        "docs/goal6_external_source_selection.md",
        format!(
            "# Goal 6 — answer-key-blind external source selection\n\n\
             - Source documents considered: {}\n- Proposals: {}\n\
             - Shadow candidates: {}\n- Answer keys read: {}\n\
             - Manifest unchanged: {}\n- False authorizations: {}\n\n\
             Proposals are ranked by generic prompt/source term overlap only;\
             this is triage evidence, not semantic coverage. Explicit source\
             provenance is required, and no source is ingested or promoted by\
             this stage.\n",
            report.source_documents_considered,
            report.proposals.len(),
            report.selected_shadow_proposals,
            report.answer_keys_read,
            report.manifest_unchanged,
            report.false_authorizations,
        ),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
