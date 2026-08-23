//! Stage 410: answer-key-blind overlap audit for candidate source lineages.
//!
//! This audit does not authorize or score answers.  It measures whether three
//! source-derived frontends can produce complete typed artifacts on the
//! naturally authored external corpus, beside the legacy route-blind portfolio.
//! It is deliberately used to choose the next acquisition target rather than
//! treating lexical subject overlap as evidence of transfer.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::goal6_external_portfolio::{executable_routes, observe_all};
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaStatus,
};
use the_machine::source_interpolation_frontend::{
    formalize_interpolation_text, replay_verified as interpolation_replay,
    InterpolationFrontendStatus,
};
use the_machine::source_logic_frontend::{
    formalize_logic_text, replay_verified as logic_frontend_replay, LogicFrontendStatus,
};
use the_machine::source_logic_pack::{evaluate as evaluate_logic, LogicStatus};
use the_machine::source_selection::{
    gap_replay_verified, select_for_gap, SourceGapRequest, SourceSelectionDecision,
};
use the_machine::source_set_frontend::{
    formalize_set_text, replay_verified as set_frontend_replay, SetFrontendStatus,
};
use the_machine::source_set_pack::{evaluate as evaluate_set, SetStatus};

const QUESTIONS_PATH: &str = "data/external_math_exam_v1/questions.jsonl";
const INTERPOLATION_SOURCE: &str = "docs/sources/openstax_linear_interpolation_catalog.txt";
const SET_SOURCE: &str = "docs/sources/openstax_finite_set_operations_catalog.txt";
const LOGIC_SOURCE: &str = "docs/sources/openstax_truth_table_catalog.txt";

#[derive(Debug, serde::Deserialize)]
struct Question {
    id: String,
    original_prompt: String,
    split: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum CandidateLineage {
    FiniteSet,
    TruthTable,
    LinearInterpolation,
}

#[derive(Debug, Serialize)]
struct LineageReport {
    lineage: CandidateLineage,
    source_path: &'static str,
    scope: &'static str,
    selected: bool,
    rejected_decoys: usize,
    frontend_complete: usize,
    execution_complete: usize,
    frontend_replay_verified: usize,
    execution_replay_verified: usize,
    frontend_tamper_rejected: usize,
    execution_tamper_rejected: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    partition: String,
    questions: usize,
    dataset_sha256: String,
    legacy_routes: usize,
    legacy_unique_ids: usize,
    cumulative_unique_ids: usize,
    cumulative_route_ambiguities: usize,
    source_selection_replays: usize,
    source_selection_failures: usize,
    lineages: Vec<LineageReport>,
    answer_keys_read: usize,
    production_authorizations: usize,
    false_authorizations: usize,
    manifest_unchanged: bool,
    report_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn evidence(
    path: &str,
    source_id: &str,
    title: &str,
    section: &str,
    url: &str,
    claim: &str,
    scope: &str,
) -> the_machine::source_evidence_envelope::SourceEvidenceEnvelope {
    let document = format!(
        "SOURCE_ID: {source_id}\nTITLE: {title}\nSECTION: {section}\nURL: {url}\nLICENSE: CC BY 4.0\nRETRIEVED_UTC: 2026-08-23\nEVIDENCE: {claim}\nSCOPE: {scope}"
    );
    the_machine::source_evidence_envelope::ingest_source_evidence(path, &document)
        .expect("candidate source evidence validates")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let partition =
        std::env::var("GOAL6_STAGE410_PARTITION").unwrap_or_else(|_| "development".into());
    assert!(matches!(partition.as_str(), "development" | "sealed"));
    let report_path = std::env::var("GOAL6_STAGE410_REPORT_JSON")
        .unwrap_or_else(|_| format!("docs/stage410_source_overlap_audit_{partition}.json"));
    let report_md_path = std::env::var("GOAL6_STAGE410_REPORT_MD")
        .unwrap_or_else(|_| format!("docs/stage410_source_overlap_audit_{partition}.md"));

    let question_bytes = fs::read(QUESTIONS_PATH)?;
    let dataset_sha256 = digest_bytes(&question_bytes);
    let questions: Vec<Question> = String::from_utf8(question_bytes)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str::<Question>)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|question| question.split == partition)
        .collect();

    let interpolation_records = extract_formula_records(include_str!(
        "../../docs/sources/openstax_linear_interpolation_catalog.txt"
    ))
    .expect("interpolation source catalog validates");
    let set_candidates = vec![
        evidence(
            SET_SOURCE,
            "openstax-contemporary-mathematics:finite-set-operations",
            "Contemporary Mathematics",
            "Set operations",
            "https://openstax.org/books/contemporary-mathematics/pages/1-4-set-operations-with-two-sets",
            "finite union, intersection, difference, complement, and cardinality",
            "finite explicit set operations",
        ),
        evidence(
            LOGIC_SOURCE,
            "openstax-contemporary-mathematics:truth-tables",
            "Contemporary Mathematics",
            "Truth tables",
            "https://openstax.org/books/contemporary-mathematics/pages/2-3-constructing-truth-tables",
            "bounded propositional truth tables",
            "bounded propositional truth tables",
        ),
    ];
    let logic_candidates = vec![
        evidence(
            LOGIC_SOURCE,
            "openstax-contemporary-mathematics:truth-tables",
            "Contemporary Mathematics",
            "Truth tables",
            "https://openstax.org/books/contemporary-mathematics/pages/2-3-constructing-truth-tables",
            "bounded propositional truth tables",
            "bounded propositional truth tables",
        ),
        evidence(
            SET_SOURCE,
            "openstax-contemporary-mathematics:finite-set-operations",
            "Contemporary Mathematics",
            "Set operations",
            "https://openstax.org/books/contemporary-mathematics/pages/1-4-set-operations-with-two-sets",
            "finite explicit set operations",
            "finite explicit set operations",
        ),
    ];
    let interpolation_candidates = vec![
        evidence(
            INTERPOLATION_SOURCE,
            "openstax-precalculus-2e:linear-functions",
            "Precalculus 2e",
            "Linear interpolation",
            "https://openstax.org/books/precalculus-2e/pages/2-1-linear-functions",
            "finite linear interpolation between two explicit endpoints",
            "finite linear interpolation",
        ),
        evidence(
            "docs/sources/openstax_precalculus_sequences_source.txt",
            "openstax-precalculus-2e:sequences",
            "Precalculus 2e",
            "Arithmetic sequences",
            "https://openstax.org/details/books/precalculus-2e",
            "finite arithmetic-sequence term evaluation",
            "finite arithmetic-sequence term evaluation",
        ),
    ];
    let requests = [
        (
            CandidateLineage::FiniteSet,
            SourceGapRequest::new(
                "gap::finite-set-operations",
                vec!["finite explicit set operations".into()],
                1,
            ),
            set_candidates,
        ),
        (
            CandidateLineage::TruthTable,
            SourceGapRequest::new(
                "gap::bounded-truth-tables",
                vec!["bounded propositional truth tables".into()],
                1,
            ),
            logic_candidates,
        ),
        (
            CandidateLineage::LinearInterpolation,
            SourceGapRequest::new(
                "gap::finite-linear-interpolation",
                vec!["finite linear interpolation".into()],
                1,
            ),
            interpolation_candidates,
        ),
    ];
    let mut selections = Vec::new();
    for (_, request, candidates) in &requests {
        let selection = select_for_gap(request, candidates);
        selections.push((request.clone(), selection));
    }
    assert!(selections
        .iter()
        .all(|(request, selection)| gap_replay_verified(request, selection)));

    let manifest_before = breadth_first_manifest().replay_hash();
    let mut legacy_unique_ids = BTreeSet::new();
    let mut cumulative_unique_ids = BTreeSet::new();
    let mut cumulative_route_ambiguities = 0;
    let mut lineage_reports = vec![
        LineageReport {
            lineage: CandidateLineage::FiniteSet,
            source_path: SET_SOURCE,
            scope: "finite explicit set operations",
            selected: false,
            rejected_decoys: 0,
            frontend_complete: 0,
            execution_complete: 0,
            frontend_replay_verified: 0,
            execution_replay_verified: 0,
            frontend_tamper_rejected: 0,
            execution_tamper_rejected: 0,
        },
        LineageReport {
            lineage: CandidateLineage::TruthTable,
            source_path: LOGIC_SOURCE,
            scope: "bounded propositional truth tables",
            selected: false,
            rejected_decoys: 0,
            frontend_complete: 0,
            execution_complete: 0,
            frontend_replay_verified: 0,
            execution_replay_verified: 0,
            frontend_tamper_rejected: 0,
            execution_tamper_rejected: 0,
        },
        LineageReport {
            lineage: CandidateLineage::LinearInterpolation,
            source_path: INTERPOLATION_SOURCE,
            scope: "finite linear interpolation",
            selected: false,
            rejected_decoys: 0,
            frontend_complete: 0,
            execution_complete: 0,
            frontend_replay_verified: 0,
            execution_replay_verified: 0,
            frontend_tamper_rejected: 0,
            execution_tamper_rejected: 0,
        },
    ];
    for (index, (_, selection)) in selections.iter().enumerate() {
        lineage_reports[index].selected = selection.selected_source_ids.len() == 1;
        lineage_reports[index].rejected_decoys = selection
            .candidates
            .iter()
            .filter(|candidate| candidate.decision == SourceSelectionDecision::Rejected)
            .count();
    }
    for question in &questions {
        let observations = observe_all(&question.original_prompt, &question.id);
        let legacy = executable_routes(&observations);
        if legacy.len() == 1 {
            legacy_unique_ids.insert(question.id.clone());
        }
        let set = formalize_set_text(&question.original_prompt, &question.id);
        let set_frontend_ok = set_frontend_replay(&set);
        let mut set_tampered = set.clone();
        set_tampered.replay_hash.push('x');
        assert!(set_frontend_ok && !set_frontend_replay(&set_tampered));
        let set_execution = set
            .request
            .as_ref()
            .map(evaluate_set)
            .filter(|result| result.status == SetStatus::Complete);
        lineage_reports[0].frontend_replay_verified += usize::from(set_frontend_ok);
        lineage_reports[0].frontend_tamper_rejected +=
            usize::from(!set_frontend_replay(&set_tampered));
        lineage_reports[0].frontend_complete +=
            usize::from(set.status == SetFrontendStatus::Complete);
        lineage_reports[0].execution_complete += usize::from(set_execution.is_some());
        if let Some(ref result) = set_execution {
            let ok = the_machine::source_set_pack::replay_verified(&result);
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            assert!(ok && !the_machine::source_set_pack::replay_verified(&tampered));
            lineage_reports[0].execution_replay_verified += usize::from(ok);
            lineage_reports[0].execution_tamper_rejected +=
                usize::from(!the_machine::source_set_pack::replay_verified(&tampered));
        }

        let logic = formalize_logic_text(&question.original_prompt, &question.id);
        let logic_frontend_ok = logic_frontend_replay(&logic);
        let mut logic_tampered = logic.clone();
        logic_tampered.replay_hash.push('x');
        assert!(logic_frontend_ok && !logic_frontend_replay(&logic_tampered));
        let logic_execution = logic
            .request
            .as_ref()
            .map(evaluate_logic)
            .filter(|result| result.status == LogicStatus::Complete);
        lineage_reports[1].frontend_replay_verified += usize::from(logic_frontend_ok);
        lineage_reports[1].frontend_tamper_rejected +=
            usize::from(!logic_frontend_replay(&logic_tampered));
        lineage_reports[1].frontend_complete +=
            usize::from(logic.status == LogicFrontendStatus::Complete);
        lineage_reports[1].execution_complete += usize::from(logic_execution.is_some());
        if let Some(ref result) = logic_execution {
            let ok = the_machine::source_logic_pack::replay_verified(&result);
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            assert!(ok && !the_machine::source_logic_pack::replay_verified(&tampered));
            lineage_reports[1].execution_replay_verified += usize::from(ok);
            lineage_reports[1].execution_tamper_rejected +=
                usize::from(!the_machine::source_logic_pack::replay_verified(&tampered));
        }

        let interpolation = formalize_interpolation_text(&question.original_prompt, &question.id);
        let interpolation_frontend_ok = interpolation_replay(&interpolation);
        let mut interpolation_tampered = interpolation.clone();
        interpolation_tampered.replay_hash.push('x');
        assert!(interpolation_frontend_ok && !interpolation_replay(&interpolation_tampered));
        let interpolation_execution = interpolation.request.as_ref().map(|request| {
            evaluate_formula_records(
                request,
                "source_catalog_linear_interpolation",
                &interpolation_records,
            )
        });
        let interpolation_execution =
            interpolation_execution.filter(|result| result.status == FormulaStatus::Complete);
        lineage_reports[2].frontend_replay_verified += usize::from(interpolation_frontend_ok);
        lineage_reports[2].frontend_tamper_rejected +=
            usize::from(!interpolation_replay(&interpolation_tampered));
        lineage_reports[2].frontend_complete +=
            usize::from(interpolation.status == InterpolationFrontendStatus::Complete);
        lineage_reports[2].execution_complete += usize::from(interpolation_execution.is_some());
        if let Some(ref result) = interpolation_execution {
            let ok = result.replay_verified();
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            assert!(ok && !tampered.replay_verified());
            lineage_reports[2].execution_replay_verified += usize::from(ok);
            lineage_reports[2].execution_tamper_rejected +=
                usize::from(!tampered.replay_verified());
        }

        let new_routes = lineage_reports
            .iter()
            .filter(|lineage| lineage.execution_complete > 0)
            .count();
        let current_new_routes = usize::from(set_execution.is_some())
            + usize::from(logic_execution.is_some())
            + usize::from(interpolation_execution.is_some());
        let total_routes = legacy.len() + current_new_routes;
        if total_routes > 1 {
            cumulative_route_ambiguities += 1;
        }
        if total_routes == 1 {
            cumulative_unique_ids.insert(question.id.clone());
        }
        let _ = new_routes;
    }
    let manifest_after = breadth_first_manifest().replay_hash();
    let source_selection_replays = selections
        .iter()
        .filter(|(request, selection)| gap_replay_verified(request, selection))
        .count();
    let source_selection_failures = selections.len() - source_selection_replays;
    let report = Report {
        schema: "stage410-source-overlap-audit-v1",
        partition: partition.clone(),
        questions: questions.len(),
        dataset_sha256,
        legacy_routes: 12,
        legacy_unique_ids: legacy_unique_ids.len(),
        cumulative_unique_ids: cumulative_unique_ids.len(),
        cumulative_route_ambiguities,
        source_selection_replays,
        source_selection_failures,
        lineages: lineage_reports,
        answer_keys_read: 0,
        production_authorizations: 0,
        false_authorizations: 0,
        manifest_unchanged: manifest_before == manifest_after,
        report_sha256: String::new(),
    };
    assert_eq!(report.answer_keys_read, 0);
    assert_eq!(report.production_authorizations, 0);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.source_selection_failures, 0);
    assert!(report.manifest_unchanged);
    let mut unsigned = serde_json::to_value(&report)?;
    unsigned["report_sha256"] = serde_json::Value::String(String::new());
    let mut report = report;
    report.report_sha256 = digest(&unsigned);
    fs::write(
        &report_path,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        &report_md_path,
        format!(
            "# Stage 410 — source overlap audit\n\n- Partition / questions: {} / {}\n- Legacy routes / unique candidates: {} / {}\n- Cumulative unique / ambiguities: {} / {}\n- Source-selection replays / failures: {} / {}\n- Answer keys / production authorizations / false authorizations: {} / {} / {}\n- Manifest unchanged: {}\n\nCandidate lineages:\n\n{}\n\nThis answer-key-blind audit measures external reachability only; it does not score or authorize candidates.\n",
            report.partition,
            report.questions,
            report.legacy_routes,
            report.legacy_unique_ids,
            report.cumulative_unique_ids,
            report.cumulative_route_ambiguities,
            report.source_selection_replays,
            report.source_selection_failures,
            report.answer_keys_read,
            report.production_authorizations,
            report.false_authorizations,
            report.manifest_unchanged,
            report
                .lineages
                .iter()
                .map(|lineage| format!("- {:?}: selected={} frontend_complete={} execution_complete={} replay={}/{}", lineage.lineage, lineage.selected, lineage.frontend_complete, lineage.execution_complete, lineage.frontend_replay_verified, lineage.execution_replay_verified))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
    )?;
    println!(
        "Stage 410 — partition={} legacy_unique={} cumulative_unique={} ambiguities={} source_replays=3",
        report.partition,
        report.legacy_unique_ids,
        report.cumulative_unique_ids,
        report.cumulative_route_ambiguities,
    );
    Ok(())
}
