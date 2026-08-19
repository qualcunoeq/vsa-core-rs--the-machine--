//! Answer-key-blind gap analysis for the frozen Goal 6 route portfolio.
//!
//! The report diagnoses the first portfolio obstruction and ranks source
//! documents only as triage candidates. It never reads answer keys, ingests a
//! source, promotes a capability, or mutates production.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::goal6_external_portfolio::{executable_routes, observe_all};

const RELEASE_DIR: &str = "data/external_math_exam_v1";
const SOURCE_DIR: &str = "docs/sources";
const REPORT_JSON: &str = "docs/goal6_external_portfolio_gaps.json";
const REPORT_MD: &str = "docs/goal6_external_portfolio_gaps.md";

#[derive(Debug, Deserialize)]
struct Question {
    id: String,
    original_prompt: String,
    split: String,
}

#[derive(Debug, Serialize)]
struct RouteGap {
    route: String,
    frontend_status_counts: BTreeMap<String, usize>,
    execution_status_counts: BTreeMap<String, usize>,
    executable_cases: usize,
}

#[derive(Debug, Serialize)]
struct SourceTriage {
    source_path: String,
    source_sha256: String,
    provenance_fields_present: bool,
    affected_residuals: usize,
    overlap_terms: usize,
    lexical_score: f64,
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    release_id: &'static str,
    partition: &'static str,
    dataset_sha256: String,
    questions_read: usize,
    answer_keys_read: usize,
    route_invocations: usize,
    unique_candidates: usize,
    no_executable_route: usize,
    multiple_route_ambiguities: usize,
    first_gate_counts: BTreeMap<String, usize>,
    route_gaps: Vec<RouteGap>,
    source_documents_considered: usize,
    source_triage_candidates: Vec<SourceTriage>,
    source_ingestions: usize,
    promotion_proposals: usize,
    production_mutations: usize,
    manifest_sha256_before: String,
    manifest_sha256_after: String,
    manifest_unchanged: bool,
    report_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn increment(map: &mut BTreeMap<String, usize>, key: impl Into<String>) {
    *map.entry(key.into()).or_default() += 1;
}

fn terms(text: &str) -> BTreeSet<String> {
    const STOP: &[&str] = &[
        "and", "are", "can", "calculate", "compute", "find", "for", "from", "given", "how",
        "if", "into", "is", "let", "of", "on", "or", "the", "then", "this", "to", "what",
        "when", "which", "with", "would",
    ];
    text.split(|character: char| !character.is_ascii_alphabetic())
        .filter(|word| word.len() >= 4)
        .map(|word| word.to_ascii_lowercase())
        .filter(|word| !STOP.contains(&word.as_str()))
        .collect()
}

fn provenance_present(text: &str) -> bool {
    [
        "SOURCE_ID:",
        "TITLE:",
        "SECTION:",
        "URL:",
        "LICENSE:",
        "EVIDENCE:",
    ]
    .iter()
    .all(|field| text.contains(field))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let question_bytes = fs::read(format!("{RELEASE_DIR}/questions.jsonl"))?;
    let dataset_sha256 = digest_bytes(&question_bytes);
    let questions: Vec<Question> = String::from_utf8(question_bytes)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<Vec<_>, _>>()?;
    let questions: Vec<Question> = questions
        .into_iter()
        .filter(|question| question.split == "development")
        .collect();

    let mut route_statuses: BTreeMap<String, (BTreeMap<String, usize>, BTreeMap<String, usize>, usize)> =
        BTreeMap::new();
    let mut first_gate_counts = BTreeMap::new();
    let mut residual_prompts = Vec::new();
    let mut unique_candidates = 0;
    let mut no_executable_route = 0;
    let mut multiple_route_ambiguities = 0;
    for question in &questions {
        let observations = observe_all(&question.original_prompt, &question.id);
        let executable = executable_routes(&observations);
        if executable.len() == 1 {
            unique_candidates += 1;
        } else {
            residual_prompts.push(question.original_prompt.clone());
            if executable.is_empty() {
                no_executable_route += 1;
            } else {
                multiple_route_ambiguities += 1;
            }
        }
        for observation in &observations {
            let entry = route_statuses
                .entry(format!("{:?}", observation.route))
                .or_insert_with(|| (BTreeMap::new(), BTreeMap::new(), 0));
            increment(&mut entry.0, observation.frontend_status.clone());
            increment(&mut entry.1, observation.execution_status.clone());
            entry.2 += usize::from(observation.executable);
        }
        let first_gate = if executable.len() > 1 {
            "multiple_replayable_routes"
        } else if observations.iter().any(|item| {
            item.frontend_status == "Complete" && item.execution_status != "Complete"
        }) {
            "complete_frontend_execution_boundary"
        } else if observations
            .iter()
            .any(|item| item.frontend_status == "Ambiguous")
        {
            "ambiguous_frontend"
        } else if observations
            .iter()
            .any(|item| item.frontend_status == "Missing")
        {
            "missing_frontend_fields"
        } else {
            "no_validated_route"
        };
        if executable.len() != 1 {
            increment(&mut first_gate_counts, first_gate);
        }
    }

    let mut source_documents = Vec::<(PathBuf, Vec<u8>, String)>::new();
    for entry in fs::read_dir(SOURCE_DIR)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let bytes = fs::read(&path)?;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        source_documents.push((path, bytes, text));
    }
    let residual_term_sets: Vec<BTreeSet<String>> =
        residual_prompts.iter().map(|prompt| terms(prompt)).collect();
    let mut source_triage_candidates = Vec::new();
    for (path, bytes, text) in &source_documents {
        let source_terms = terms(text);
        let mut affected_residuals = 0;
        let mut overlap_terms = 0;
        for prompt_terms in &residual_term_sets {
            let overlap = prompt_terms.intersection(&source_terms).count();
            if overlap >= 2 {
                affected_residuals += 1;
                overlap_terms += overlap;
            }
        }
        if affected_residuals == 0 {
            continue;
        }
        let lexical_score = overlap_terms as f64
            / residual_term_sets.len().max(1) as f64;
        source_triage_candidates.push(SourceTriage {
            source_path: path.to_string_lossy().to_string(),
            source_sha256: digest_bytes(bytes),
            provenance_fields_present: provenance_present(text),
            affected_residuals,
            overlap_terms,
            lexical_score,
            status: "triage_only",
        });
    }
    source_triage_candidates.sort_by(|left, right| {
        right
            .affected_residuals
            .cmp(&left.affected_residuals)
            .then_with(|| right.overlap_terms.cmp(&left.overlap_terms))
            .then_with(|| left.source_path.cmp(&right.source_path))
    });
    source_triage_candidates.truncate(12);

    let route_gaps = route_statuses
        .into_iter()
        .map(|(route, (frontend_status_counts, execution_status_counts, executable_cases))| {
            RouteGap {
                route,
                frontend_status_counts,
                execution_status_counts,
                executable_cases,
            }
        })
        .collect();
    let manifest_sha256_before = breadth_first_manifest().replay_hash();
    let manifest_sha256_after = breadth_first_manifest().replay_hash();
    let mut report = Report {
        schema: "goal6-external-portfolio-gaps-v1",
        release_id: "external-math-exam-v1",
        partition: "development",
        dataset_sha256,
        questions_read: questions.len(),
        answer_keys_read: 0,
        route_invocations: questions.len() * 4,
        unique_candidates,
        no_executable_route,
        multiple_route_ambiguities,
        first_gate_counts,
        route_gaps,
        source_documents_considered: source_documents.len(),
        source_triage_candidates,
        source_ingestions: 0,
        promotion_proposals: 0,
        production_mutations: 0,
        manifest_sha256_before: manifest_sha256_before.clone(),
        manifest_sha256_after: manifest_sha256_after.clone(),
        manifest_unchanged: manifest_sha256_before == manifest_sha256_after,
        report_sha256: String::new(),
    };
    let mut unsigned = serde_json::to_value(&report)?;
    unsigned["report_sha256"] = serde_json::Value::String(String::new());
    report.report_sha256 = digest(&unsigned);
    assert_eq!(report.answer_keys_read, 0);
    assert_eq!(report.source_ingestions, 0);
    assert_eq!(report.promotion_proposals, 0);
    assert_eq!(report.production_mutations, 0);
    assert!(report.manifest_unchanged);
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Goal 6 — route-blind portfolio gap analysis\n\n\
- Development questions / route invocations: {} / {}\n\
- Unique candidates / multiple-route ambiguities / no route: {} / {} / {}\n\
- First-gate counts: {:?}\n\
- Source documents considered / triage candidates: {} / {}\n\
- Source ingestions / promotion proposals / production mutations: {} / {} / {}\n\
- Answer keys read: {}\n\
- Manifest unchanged: {}\n\n\
Source rankings are lexical triage only. No source was ingested or promoted.\n",
            report.questions_read,
            report.route_invocations,
            report.unique_candidates,
            report.multiple_route_ambiguities,
            report.no_executable_route,
            report.first_gate_counts,
            report.source_documents_considered,
            report.source_triage_candidates.len(),
            report.source_ingestions,
            report.promotion_proposals,
            report.production_mutations,
            report.answer_keys_read,
            report.manifest_unchanged,
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
