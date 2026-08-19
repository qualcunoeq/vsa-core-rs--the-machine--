//! Governed source-plan generation for the Goal 6 external portfolio.
//!
//! This stage consumes only the answer-key-blind portfolio gap report and
//! source-document metadata.  It creates a review queue, not a knowledge pack:
//! lexical overlap is never treated as semantic coverage, and every candidate
//! remains blocked until an independently authored exercise corpus and source
//! validation receipts exist.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;

const GAP_REPORT: &str = "docs/goal6_external_portfolio_gaps.json";
const PLAN_JSON: &str = "docs/goal6_external_portfolio_source_plan.json";
const PLAN_MD: &str = "docs/goal6_external_portfolio_source_plan.md";

#[derive(Debug, Deserialize)]
struct GapReport {
    schema: String,
    report_sha256: String,
    dataset_sha256: String,
    questions_read: usize,
    answer_keys_read: usize,
    source_documents_considered: usize,
    source_triage_candidates: Vec<SourceTriage>,
    source_ingestions: usize,
    promotion_proposals: usize,
    production_mutations: usize,
    manifest_unchanged: bool,
    route_gaps: Vec<RouteGap>,
}

#[derive(Debug, Deserialize)]
struct RouteGap {
    route: String,
    executable_cases: usize,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct SourceTriage {
    source_path: String,
    source_sha256: String,
    provenance_fields_present: bool,
    affected_residuals: usize,
    overlap_terms: usize,
    lexical_score: f64,
    status: String,
}

#[derive(Debug, Serialize)]
struct SourcePlanEntry {
    source_path: String,
    source_sha256: String,
    provenance_fields_present: bool,
    affected_residuals: usize,
    overlap_terms: usize,
    lexical_score: f64,
    matching_route: Option<String>,
    executable_cases: usize,
    semantic_gate: &'static str,
    queue_rank: usize,
    decision: &'static str,
    semantic_validation_required: bool,
    independent_exercises_required: bool,
    ingestion_allowed: bool,
    promotion_allowed: bool,
    blocking_reasons: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    release_id: &'static str,
    input_gap_schema: String,
    input_gap_report_sha256: String,
    dataset_sha256: String,
    development_questions_read: usize,
    answer_keys_read: usize,
    source_documents_considered: usize,
    triage_candidates_read: usize,
    review_queue_size: usize,
    semantic_ready_entries: usize,
    plan_entries: Vec<SourcePlanEntry>,
    source_ingestions: usize,
    promotion_proposals: usize,
    production_mutations: usize,
    manifest_sha256_before: String,
    manifest_sha256_after: String,
    manifest_unchanged: bool,
    all_entries_blocked: bool,
    report_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn report_digest(report: &Report) -> String {
    let mut unsigned = serde_json::to_value(report).expect("source plan serializes");
    unsigned["report_sha256"] = serde_json::Value::String(String::new());
    digest(&unsigned)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let gap_bytes = fs::read(GAP_REPORT)?;
    let gap: GapReport = serde_json::from_slice(&gap_bytes)?;
    assert_eq!(gap.schema, "goal6-external-portfolio-gaps-v1");
    assert_eq!(gap.answer_keys_read, 0);
    assert_eq!(gap.source_ingestions, 0);
    assert_eq!(gap.promotion_proposals, 0);
    assert_eq!(gap.production_mutations, 0);
    assert!(gap.manifest_unchanged);

    // Keep the queue deliberately small and deterministic.  This is a review
    // queue based on lexical triage, never a claim of semantic applicability.
    let triage_candidates_read = gap.source_triage_candidates.len();
    let mut triage = gap
        .source_triage_candidates
        .into_iter()
        .filter(|candidate| candidate.status == "triage_only")
        .collect::<Vec<_>>();
    triage.sort_by(|left, right| {
        right
            .affected_residuals
            .cmp(&left.affected_residuals)
            .then_with(|| right.overlap_terms.cmp(&left.overlap_terms))
            .then_with(|| left.source_path.cmp(&right.source_path))
    });
    triage.truncate(5);

    // A source is actionable only when the validated route that would consume
    // it has at least one complete executable residual. This prevents lexical
    // overlap from becoming a de facto curriculum decision.
    let route_evidence = gap
        .route_gaps
        .iter()
        .map(|route| (route.route.as_str(), route.executable_cases))
        .collect::<std::collections::BTreeMap<_, _>>();

    fn route_for_source(path: &str) -> Option<&'static str> {
        if path.contains("bounded_geometry") {
            Some("BoundedGeometry")
        } else if path.contains("finite_regression") {
            Some("FiniteRegression")
        } else if path.contains("finite_statistics") {
            Some("FiniteStatistics")
        } else {
            None
        }
    }

    let plan_entries = triage
        .into_iter()
        .enumerate()
        .map(|(index, candidate)| {
            let matching_route = route_for_source(&candidate.source_path);
            let executable_cases = matching_route
                .and_then(|route| route_evidence.get(route).copied())
                .unwrap_or(0);
            let semantic_gate = if executable_cases > 0 {
                "complete_route_evidence"
            } else if matching_route.is_some() {
                "lexical_only_no_complete_route"
            } else {
                "no_matching_validated_route"
            };
            let mut blocking_reasons = vec![
                "lexical_overlap_is_not_semantic_coverage",
                "independent_exercise_corpus_missing",
                "source_scope_and_authority_require_review",
            ];
            if executable_cases == 0 {
                blocking_reasons.push("no_complete_executable_residual_for_source_route");
            }
            SourcePlanEntry {
                source_path: candidate.source_path,
                source_sha256: candidate.source_sha256,
                provenance_fields_present: candidate.provenance_fields_present,
                affected_residuals: candidate.affected_residuals,
                overlap_terms: candidate.overlap_terms,
                lexical_score: candidate.lexical_score,
                matching_route: matching_route.map(String::from),
                executable_cases,
                semantic_gate,
                queue_rank: index + 1,
                decision: "review_queue_only",
                semantic_validation_required: true,
                independent_exercises_required: true,
                ingestion_allowed: false,
                promotion_allowed: false,
                blocking_reasons,
            }
        })
        .collect::<Vec<_>>();

    let manifest_before = breadth_first_manifest().replay_hash();
    let manifest_after = breadth_first_manifest().replay_hash();
    let mut report = Report {
        schema: "goal6-external-portfolio-source-plan-v1",
        release_id: "external-math-exam-v1",
        input_gap_schema: "goal6-external-portfolio-gaps-v1".into(),
        input_gap_report_sha256: gap.report_sha256,
        dataset_sha256: gap.dataset_sha256,
        development_questions_read: gap.questions_read,
        answer_keys_read: 0,
        source_documents_considered: gap.source_documents_considered,
        triage_candidates_read,
        review_queue_size: plan_entries.len(),
        semantic_ready_entries: plan_entries
            .iter()
            .filter(|entry| entry.semantic_gate == "complete_route_evidence")
            .count(),
        plan_entries,
        source_ingestions: 0,
        promotion_proposals: 0,
        production_mutations: 0,
        manifest_sha256_before: manifest_before.clone(),
        manifest_sha256_after: manifest_after.clone(),
        manifest_unchanged: manifest_before == manifest_after,
        all_entries_blocked: true,
        report_sha256: String::new(),
    };
    report.report_sha256 = report_digest(&report);
    assert_eq!(report.answer_keys_read, 0);
    assert_eq!(report.source_ingestions, 0);
    assert_eq!(report.promotion_proposals, 0);
    assert_eq!(report.production_mutations, 0);
    assert!(report.manifest_unchanged);
    assert!(report.all_entries_blocked);

    fs::write(PLAN_JSON, format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    let queue_lines = report
        .plan_entries
        .iter()
        .map(|entry| {
            format!(
                "{}. `{}` — {} residuals, lexical score {:.3}, blocked pending semantic validation and independent exercises",
                entry.queue_rank,
                entry.source_path,
                entry.affected_residuals,
                entry.lexical_score
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        PLAN_MD,
        format!(
            "# Goal 6 — governed external source plan\n\n\
             This is an answer-key-blind review queue derived from the frozen\
             portfolio gap report. Lexical overlap is triage evidence only; no\
             source was ingested, synthesized, promoted, or routed.\n\n\
             - Development questions read: {}\n\
             - Source documents considered: {}\n\
             - Triage candidates read / review queue: {} / {}\n\
             - Semantic-ready entries: {}\n\
             - Answer keys read: {}\n\
             - Source ingestions / promotion proposals / production mutations: {} / {} / {}\n\
             - Manifest unchanged: {}\n\
             - All queue entries blocked: {}\n\n{}\n",
            report.development_questions_read,
            report.source_documents_considered,
            report.triage_candidates_read,
            report.review_queue_size,
            report.semantic_ready_entries,
            report.answer_keys_read,
            report.source_ingestions,
            report.promotion_proposals,
            report.production_mutations,
            report.manifest_unchanged,
            report.all_entries_blocked,
            queue_lines,
        ),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
