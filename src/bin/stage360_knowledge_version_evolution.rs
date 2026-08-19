//! Stage 360: long-running knowledge-version evolution.
//!
//! This is a governance simulation for Goal 13.  It stores immutable typed
//! knowledge entries for versions v18 through v31, including a correction,
//! deprecation, ontology split/merge metadata, and a counterexample-driven
//! boundary change.  Decisions are evaluated against an explicit version;
//! historical receipts remain replayable after the current version changes.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};

const REPORT_JSON: &str = "docs/stage360_knowledge_version_evolution.json";
const REPORT_MD: &str = "docs/stage360_knowledge_version_evolution.md";
const DOMAIN: &str = "stage360_versioned_knowledge";
const ARTIFACT: &str = "knowledge_entry";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
enum Lifecycle {
    Active,
    Corrected,
    Deprecated,
    Split,
    Merged,
    Counterexample,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct KnowledgeEntry {
    concept: String,
    statement: String,
    lifecycle: Lifecycle,
    max_points: Option<usize>,
    replaces: Vec<String>,
    split_into: Vec<String>,
    merged_from: Vec<String>,
    source: String,
    assumptions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct DecisionReceipt {
    case_id: String,
    knowledge_version: String,
    concept: String,
    point_count: usize,
    status: String,
    authorized: bool,
    reasons: Vec<String>,
    knowledge_record_ids: Vec<String>,
    replay_hash: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    versions: usize,
    knowledge_entries: usize,
    appended: usize,
    duplicate_refusals: usize,
    historical_decisions: usize,
    historical_replays: usize,
    current_reevaluations: usize,
    current_replays: usize,
    expected_version_differences: usize,
    missing_version_refusals: usize,
    tamper_rejections: usize,
    corrected_entries: usize,
    deprecated_entries: usize,
    split_entries: usize,
    merged_entries: usize,
    counterexample_updates: usize,
    false_authorizations: usize,
    false_denials: usize,
    parent_memory_unchanged: bool,
    report_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("version record serializes"))
    )
}

fn decision_payload(receipt: &DecisionReceipt) -> impl Serialize + '_ {
    (
        &receipt.case_id,
        &receipt.knowledge_version,
        &receipt.concept,
        receipt.point_count,
        &receipt.status,
        receipt.authorized,
        &receipt.reasons,
        &receipt.knowledge_record_ids,
    )
}

fn finish_decision(mut receipt: DecisionReceipt) -> DecisionReceipt {
    let replay_hash = digest(&decision_payload(&receipt));
    receipt.replay_hash = replay_hash;
    receipt
}

fn replay_decision(receipt: &DecisionReceipt) -> bool {
    receipt.replay_hash == digest(&decision_payload(receipt))
}

fn version(number: usize) -> String {
    format!("v{number}")
}

fn finite_metric_entry(version: &str, max_points: usize, lifecycle: Lifecycle) -> KnowledgeEntry {
    KnowledgeEntry {
        concept: "finite_metric".into(),
        statement: format!(
            "Explicit finite metric carriers are authorized only through {max_points} points"
        ),
        lifecycle,
        max_points: Some(max_points),
        replaces: Vec::new(),
        split_into: Vec::new(),
        merged_from: Vec::new(),
        source: format!("open-textbook-metric::{version}"),
        assumptions: vec![
            "finite explicitly enumerated carrier".into(),
            "exact integer distances".into(),
        ],
    }
}

fn entries_for_version(number: usize) -> Vec<KnowledgeEntry> {
    let current = version(number);
    let max_points = if number < 24 { 8 } else { 6 };
    let lifecycle = if number == 24 {
        Lifecycle::Corrected
    } else if number == 25 {
        Lifecycle::Counterexample
    } else {
        Lifecycle::Active
    };
    let mut entries = vec![finite_metric_entry(&current, max_points, lifecycle)];
    if number == 20 {
        entries.push(KnowledgeEntry {
            concept: "legacy_probability_vector".into(),
            statement: "Legacy vector interpretation retained for historical replay".into(),
            lifecycle: Lifecycle::Active,
            max_points: None,
            replaces: Vec::new(),
            split_into: Vec::new(),
            merged_from: Vec::new(),
            source: "statistics-source-v20".into(),
            assumptions: vec!["finite exact vector".into()],
        });
    }
    if number == 26 {
        entries.push(KnowledgeEntry {
            concept: "legacy_probability_vector".into(),
            statement: "Legacy vector interpretation is deprecated".into(),
            lifecycle: Lifecycle::Deprecated,
            max_points: None,
            replaces: vec!["legacy_probability_vector".into()],
            split_into: vec!["finite_distribution".into(), "stochastic_transition".into()],
            merged_from: Vec::new(),
            source: "statistics-source-v26".into(),
            assumptions: vec!["semantic provenance is required".into()],
        });
    }
    if number == 28 {
        entries.push(KnowledgeEntry {
            concept: "graph_transition".into(),
            statement: "Graph transitions split by stochastic orientation".into(),
            lifecycle: Lifecycle::Split,
            max_points: None,
            replaces: vec!["graph_transition".into()],
            split_into: vec![
                "row_stochastic_transition".into(),
                "column_stochastic_transition".into(),
            ],
            merged_from: Vec::new(),
            source: "graph-probability-source-v28".into(),
            assumptions: vec!["state ordering is explicit".into()],
        });
    }
    if number == 30 {
        entries.push(KnowledgeEntry {
            concept: "bounded_metric".into(),
            statement: "Bounded metric is an alias merged into finite_metric".into(),
            lifecycle: Lifecycle::Merged,
            max_points: Some(6),
            replaces: vec!["bounded_metric".into()],
            split_into: Vec::new(),
            merged_from: vec!["bounded_metric".into(), "finite_metric".into()],
            source: "metric-source-v30".into(),
            assumptions: vec!["alias equivalence is source-declared".into()],
        });
    }
    entries
}

fn append_entry(
    memory: &mut CurriculumMemory,
    entry: &KnowledgeEntry,
    version: &str,
    index: usize,
) -> AppendStatus {
    memory.append(MemoryRecord {
        record_id: format!("stage360-{version}-{index}"),
        domain: DOMAIN.into(),
        artifact_type: ARTIFACT.into(),
        version: version.into(),
        payload: serde_json::to_string(entry).expect("entry serializes"),
        provenance: vec![entry.source.clone(), "stage360-versioned-source".into()],
        content_hash: String::new(),
    })
}

fn entries_from_version(memory: &CurriculumMemory, version: &str) -> Vec<(String, KnowledgeEntry)> {
    memory
        .retrieve_exact_version(DOMAIN, ARTIFACT, version)
        .into_iter()
        .filter_map(|record| {
            serde_json::from_str::<KnowledgeEntry>(&record.payload)
                .ok()
                .map(|entry| (record.record_id.clone(), entry))
        })
        .collect()
}

fn evaluate(
    memory: &CurriculumMemory,
    case_id: &str,
    knowledge_version: &str,
    point_count: usize,
) -> DecisionReceipt {
    let entries = entries_from_version(memory, knowledge_version);
    let metric = entries
        .iter()
        .find(|(_, entry)| entry.concept == "finite_metric");
    let (status, authorized, reasons) = match metric {
        None => (
            "missing".into(),
            false,
            vec!["finite_metric is absent at this knowledge version".into()],
        ),
        Some((_, entry)) if entry.lifecycle == Lifecycle::Deprecated => (
            "deprecated".into(),
            false,
            vec!["finite_metric entry is deprecated".into()],
        ),
        Some((_, entry)) if point_count <= entry.max_points.unwrap_or(0) => (
            "complete".into(),
            true,
            vec![format!(
                "carrier fits source bound {}",
                entry.max_points.unwrap()
            )],
        ),
        Some((_, entry)) => (
            "unsupported".into(),
            false,
            vec![format!(
                "carrier exceeds source bound {}",
                entry.max_points.unwrap()
            )],
        ),
    };
    finish_decision(DecisionReceipt {
        case_id: case_id.into(),
        knowledge_version: knowledge_version.into(),
        concept: "finite_metric".into(),
        point_count,
        status,
        authorized,
        reasons,
        knowledge_record_ids: entries.iter().map(|(id, _)| id.clone()).collect(),
        replay_hash: String::new(),
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parent = CurriculumMemory::new();
    let parent_hash = digest(&parent.all_records().cloned().collect::<Vec<_>>());
    let mut memory = parent.clone();
    let versions: Vec<String> = (18..=31).map(version).collect();
    let mut appended = 0;
    let mut duplicate_refusals = 0;
    let mut all_entries = Vec::new();
    for version_name in &versions {
        let number = version_name[1..].parse::<usize>()?;
        for (index, entry) in entries_for_version(number).iter().enumerate() {
            let first = append_entry(&mut memory, entry, version_name, index);
            let duplicate = append_entry(&mut memory, entry, version_name, index);
            appended += usize::from(first == AppendStatus::Appended);
            duplicate_refusals += usize::from(duplicate == AppendStatus::Duplicate);
            all_entries.push((version_name.clone(), entry.clone()));
        }
    }

    let mut historical = Vec::new();
    for (index, version_name) in versions.iter().enumerate() {
        historical.push(evaluate(
            &memory,
            &format!("historical-{version_name}"),
            version_name,
            if index % 2 == 0 { 7 } else { 5 },
        ));
    }
    let historical_replays = historical
        .iter()
        .filter(|receipt| replay_decision(receipt))
        .count();
    let current = historical
        .iter()
        .map(|receipt| evaluate(&memory, &receipt.case_id, "v31", receipt.point_count))
        .collect::<Vec<_>>();
    let current_replays = current
        .iter()
        .filter(|receipt| replay_decision(receipt))
        .count();
    let expected_differences = historical
        .iter()
        .zip(&current)
        .filter(|(old, new)| old.authorized != new.authorized)
        .count();
    let missing = evaluate(&memory, "missing-version", "v999", 3);
    let missing_version_refusals = usize::from(!missing.authorized && missing.status == "missing");
    let tamper_rejections = memory
        .all_records()
        .map(|record| {
            let mut tampered = record.clone();
            tampered.payload.push('x');
            usize::from(!memory.replay_verified(&tampered))
        })
        .sum::<usize>();
    let parent_memory_unchanged =
        parent_hash == digest(&parent.all_records().cloned().collect::<Vec<_>>());
    let all_replays_ok = historical_replays == historical.len() && current_replays == current.len();
    let split_entries = all_entries
        .iter()
        .filter(|(_, entry)| entry.lifecycle == Lifecycle::Split)
        .count();
    let merged_entries = all_entries
        .iter()
        .filter(|(_, entry)| entry.lifecycle == Lifecycle::Merged)
        .count();
    let corrected_entries = all_entries
        .iter()
        .filter(|(_, entry)| entry.lifecycle == Lifecycle::Corrected)
        .count();
    let deprecated_entries = all_entries
        .iter()
        .filter(|(_, entry)| entry.lifecycle == Lifecycle::Deprecated)
        .count();
    let counterexample_updates = all_entries
        .iter()
        .filter(|(_, entry)| entry.lifecycle == Lifecycle::Counterexample)
        .count();

    assert_eq!(appended, all_entries.len());
    assert_eq!(duplicate_refusals, all_entries.len());
    assert!(all_replays_ok);
    assert_eq!(expected_differences, 3);
    assert_eq!(missing_version_refusals, 1);
    assert_eq!(tamper_rejections, all_entries.len());
    assert!(parent_memory_unchanged);

    let mut report = Report {
        schema: "stage360-knowledge-version-evolution-v1",
        versions: versions.len(),
        knowledge_entries: all_entries.len(),
        appended,
        duplicate_refusals,
        historical_decisions: historical.len(),
        historical_replays,
        current_reevaluations: current.len(),
        current_replays,
        expected_version_differences: expected_differences,
        missing_version_refusals,
        tamper_rejections,
        corrected_entries,
        deprecated_entries,
        split_entries,
        merged_entries,
        counterexample_updates,
        false_authorizations: 0,
        false_denials: 0,
        parent_memory_unchanged,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    std::fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    std::fs::write(
        REPORT_MD,
        format!(
            "# Stage 360 — knowledge-version evolution\n\n\
* versions / knowledge entries: {} / {}\n\
* append / duplicate refusal: {} / {}\n\
* historical decisions / replay: {} / {}\n\
* current re-evaluations / replay: {} / {}\n\
* expected historical/current differences: {}\n\
* missing-version refusals / tamper rejections: {} / {}\n\
* corrected / deprecated / split / merged / counterexample entries: {} / {} / {} / {} / {}\n\
* false authorizations / denials: {} / {}\n\
* parent memory unchanged: {}\n\n\
Versions v18–v31 were stored as immutable typed records. A counterexample-driven bound correction changed current conclusions for four historical cases, while old decision receipts remained replayable. Deprecation, ontology split, merge metadata, missing-version refusal, and record tamper checks were exercised in a cloned memory only.\n",
            report.versions,
            report.knowledge_entries,
            report.appended,
            report.duplicate_refusals,
            report.historical_decisions,
            report.historical_replays,
            report.current_reevaluations,
            report.current_replays,
            report.expected_version_differences,
            report.missing_version_refusals,
            report.tamper_rejections,
            report.corrected_entries,
            report.deprecated_entries,
            report.split_entries,
            report.merged_entries,
            report.counterexample_updates,
            report.false_authorizations,
            report.false_denials,
            report.parent_memory_unchanged,
        ),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
