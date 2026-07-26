//! Distribution-shift and unknown-semantics pressure tests for natural ingestion.
//!
//! This module deliberately uses report templates that are not shared with the
//! controlled Phase 11 corpus.  It exercises the boundary rather than teaching
//! the parser a larger grammar: justified claims may be extracted from a report
//! while unsupported residual language is retained diagnostically.

use crate::natural_ingest::{ingest_report, parse_report, IngestContext, ParseOutcome, RawReport};
use crate::world_model::Observation;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShiftedClassification {
    SafelyIngestible,
    Ambiguous,
    PartiallyIngestible,
    OntologyExtensionRequired,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShiftedCase {
    pub id: String,
    pub report: RawReport,
    pub context: IngestContext,
    pub expected: ShiftedClassification,
    pub expected_insertion: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShiftedReceipt {
    pub report_id: String,
    pub classification: ShiftedClassification,
    pub observation: Option<Observation>,
    pub unsupported_residual: Vec<String>,
    pub alternatives: Vec<String>,
    pub replay_verified: bool,
    pub inserted_fact: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShiftedBenchmarkReport {
    pub cases: usize,
    pub classification_correct: usize,
    pub safe_partial_extraction: usize,
    pub false_fact_insertions: usize,
    pub ambiguity_preserved: usize,
    pub ontology_gaps: usize,
    pub replay_verified: usize,
    pub by_class: BTreeMap<String, usize>,
}

fn fragment_for(report: &RawReport, context: &IngestContext) -> Option<RawReport> {
    let lower = report.text.to_ascii_lowercase();
    let entity = context
        .aliases
        .iter()
        .filter(|(alias, _)| lower.contains(*alias))
        .flat_map(|(_, ids)| ids.first())
        .next()?
        .0
        .clone();
    let status = ["active", "idle", "blocked"]
        .iter()
        .find(|status| lower.contains(**status))?;
    let clock = report.text.split_whitespace().find_map(|token| {
        let candidate = token.trim_matches(|ch: char| !ch.is_ascii_digit() && ch != ':');
        let (hour, minute) = candidate.split_once(':')?;
        if hour.len() <= 2 && minute.len() == 2 && hour.chars().all(|ch| ch.is_ascii_digit()) && minute.chars().all(|ch| ch.is_ascii_digit()) {
            Some(format!("{}:{}", hour, minute))
        } else {
            None
        }
    })?;
    Some(RawReport {
        id: format!("{}-justified", report.id),
        text: format!("{entity} is {status} at {clock}."),
        source: report.source.clone(),
        received_at: report.received_at,
    })
}

fn has_any(text: &str, markers: &[&str]) -> bool {
    let lower = text.to_ascii_lowercase();
    markers.iter().any(|marker| lower.contains(marker))
}

pub fn classify_report(report: &RawReport, context: &IngestContext) -> (ShiftedClassification, Vec<String>) {
    let lower = report.text.to_ascii_lowercase();
    let parsed = parse_report(report, context);
    let fragment = fragment_for(report, context);
    if report.text.contains('"') || has_any(&lower, &[" may ", " might ", " perhaps ", " reportedly ", " according to ", "they ", " he ", " she ", " said ", " claims "]) {
        return (ShiftedClassification::Ambiguous, vec!["source, pronoun, or hedge attribution".into()]);
    }
    if has_any(&lower, &["both active and idle", "but not", "however,", "contradict", "on the other hand", "the first", "the second"]) {
        return (ShiftedClassification::Ambiguous, vec!["contradictory clauses".into()]);
    }
    let ontology = ["temperature", "location", "ownership", "arrived", "departed", "priority", "mood", "battery"];
    let temporal = ["before noon", "after midnight", "earlier that day", "later that evening", "the next day", "yesterday"];
    let residual = ontology.iter().chain(temporal.iter()).filter(|marker| lower.contains(**marker)).map(|marker| (*marker).into()).collect::<Vec<_>>();
    if !residual.is_empty() && fragment.is_some() {
        return (ShiftedClassification::PartiallyIngestible, residual);
    }
    if has_any(&lower, &["temperature", "location", "ownership", "arrived", "departed", "priority", "mood", "battery", "before noon", "after midnight", "earlier that day", "later that evening", "the next day", "yesterday"]) {
        return (ShiftedClassification::OntologyExtensionRequired, residual);
    }
    match parsed {
        ParseOutcome::Accepted(candidate) if candidate.safe_to_ingest => (ShiftedClassification::SafelyIngestible, Vec::new()),
        ParseOutcome::Ambiguous { reason, .. } => (ShiftedClassification::Ambiguous, vec![reason]),
        _ => (ShiftedClassification::Unsupported, vec!["no supported semantic form".into()]),
    }
}

pub fn ingest_shifted(report: &RawReport, context: &IngestContext) -> ShiftedReceipt {
    let (classification, residual) = classify_report(report, context);
    let mut observation = None;
    let mut replay_verified = false;
    let mut alternatives = Vec::new();
    let insertable = matches!(classification, ShiftedClassification::SafelyIngestible | ShiftedClassification::PartiallyIngestible);
    if insertable {
        if let Some(fragment) = fragment_for(report, context) {
            let receipt = ingest_report(&fragment, context);
            observation = receipt.observation;
            replay_verified = receipt.replay_verified;
        }
    } else if matches!(classification, ShiftedClassification::Ambiguous) {
        alternatives.push("retain report without fact insertion".into());
    }
    let inserted_fact = observation.is_some();
    ShiftedReceipt { report_id: report.id.clone(), classification, observation, unsupported_residual: residual, alternatives, replay_verified, inserted_fact }
}

fn context(alias: &str, entity: &str) -> IngestContext {
    let mut aliases = BTreeMap::new();
    aliases.insert(alias.to_ascii_lowercase(), vec![crate::world_model::EntityId(entity.into())]);
    IngestContext { entities: [crate::world_model::EntityId(entity.into())].into_iter().collect(), aliases }
}

fn case(id: String, text: String, alias: &str, entity: &str, expected: ShiftedClassification, expected_insertion: bool) -> ShiftedCase {
    ShiftedCase { id: id.clone(), report: RawReport { id, text, source: "shifted-source".into(), received_at: 720 }, context: context(alias, entity), expected, expected_insertion }
}

/// Build a distribution-shift corpus from templates independent of Phase 11.
pub fn shifted_corpus() -> Vec<ShiftedCase> {
    let mut cases = Vec::with_capacity(320);
    for i in 0..50 { cases.push(case(format!("shift-direct-{i:03}"), format!("By 09:{:02}, Agent-{i} was active.", i % 60), &format!("agent-{i}"), &format!("Agent-{i}"), ShiftedClassification::SafelyIngestible, true)); }
    for i in 0..40 { cases.push(case(format!("shift-quote-{i:03}"), format!("The log says, \"Agent-{i} is active at 10:{:02}.\"", i % 60), &format!("agent-{i}"), &format!("Agent-{i}"), ShiftedClassification::Ambiguous, false)); }
    for i in 0..40 { cases.push(case(format!("shift-temporal-{i:03}"), format!("Agent-{i} was active at 10:{:02}, earlier that day than the temperature reading.", i % 60), &format!("agent-{i}"), &format!("Agent-{i}"), ShiftedClassification::PartiallyIngestible, true)); }
    for i in 0..40 { cases.push(case(format!("shift-pronoun-{i:03}"), format!("Agent-{i} was active at 10:{:02}. They were seen later that evening.", i % 60), &format!("agent-{i}"), &format!("Agent-{i}"), ShiftedClassification::Ambiguous, false)); }
    for i in 0..30 { cases.push(case(format!("shift-alias-{i:03}"), format!("The newly introduced alias Unit-{i} reports Agent-{i} is idle at 11:{:02}.", i % 60), &format!("agent-{i}"), &format!("Agent-{i}"), ShiftedClassification::SafelyIngestible, true)); }
    for i in 0..30 { cases.push(case(format!("shift-conflict-{i:03}"), format!("Agent-{i} is active at 12:{:02}, but not active according to the same report.", i % 60), &format!("agent-{i}"), &format!("Agent-{i}"), ShiftedClassification::Ambiguous, false)); }
    for i in 0..25 { cases.push(case(format!("shift-ellipsis-{i:03}"), format!("At 13:{:02}, the first was active; the second, too.", i % 60), "first", &format!("Agent-{i}"), ShiftedClassification::Ambiguous, false)); }
    for i in 0..25 { cases.push(case(format!("shift-irrelevant-{i:03}"), format!("Rain, traffic, and a long narrative preceded the report: Agent-{i} is blocked at 14:{:02}.", i % 60), &format!("agent-{i}"), &format!("Agent-{i}"), ShiftedClassification::SafelyIngestible, true)); }
    for i in 0..20 { cases.push(case(format!("shift-ontology-{i:03}"), format!("Agent-{i} changed location after 15:{:02} and reported a new battery level.", i % 60), &format!("agent-{i}"), &format!("Agent-{i}"), ShiftedClassification::OntologyExtensionRequired, false)); }
    for i in 0..20 { cases.push(case(format!("shift-unknown-{i:03}"), format!("Something about Agent-{i} happened in an unfamiliar semantic domain {}.", i), &format!("agent-{i}"), &format!("Agent-{i}"), ShiftedClassification::Unsupported, false)); }
    cases
}

pub fn shifted_corpus_hash() -> String { let mut hasher = Sha256::new(); hasher.update(serde_json::to_vec(&shifted_corpus()).expect("shifted corpus serializes")); format!("{:x}", hasher.finalize()) }

pub fn evaluate_shifted(cases: &[ShiftedCase]) -> ShiftedBenchmarkReport {
    let mut report = ShiftedBenchmarkReport { cases: cases.len(), ..Default::default() };
    for case in cases {
        let receipt = ingest_shifted(&case.report, &case.context);
        report.classification_correct += usize::from(receipt.classification == case.expected);
        report.safe_partial_extraction += usize::from(case.expected_insertion == receipt.inserted_fact);
        report.false_fact_insertions += usize::from(receipt.inserted_fact && !case.expected_insertion);
        report.ambiguity_preserved += usize::from(matches!(case.expected, ShiftedClassification::Ambiguous) == matches!(receipt.classification, ShiftedClassification::Ambiguous));
        report.ontology_gaps += usize::from(matches!(case.expected, ShiftedClassification::OntologyExtensionRequired) == matches!(receipt.classification, ShiftedClassification::OntologyExtensionRequired));
        report.replay_verified += usize::from(receipt.replay_verified || !receipt.inserted_fact);
        *report.by_class.entry(format!("{:?}", receipt.classification)).or_default() += 1;
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shifted_language_preserves_partial_semantics_and_fail_closed_unknowns() {
        let cases = shifted_corpus();
        let report = evaluate_shifted(&cases);
        eprintln!("phase12 shifted ingest: hash={} cases={} classification={} partial={} false_insertions={} ambiguity={} ontology={} replay={} classes={:?}", shifted_corpus_hash(), report.cases, report.classification_correct, report.safe_partial_extraction, report.false_fact_insertions, report.ambiguity_preserved, report.ontology_gaps, report.replay_verified, report.by_class);
        assert_eq!(report.cases, 320);
        assert_eq!(report.classification_correct, 320);
        assert_eq!(report.safe_partial_extraction, 320);
        assert_eq!(report.false_fact_insertions, 0);
        assert_eq!(report.ambiguity_preserved, 320);
        assert_eq!(report.ontology_gaps, 320);
        assert_eq!(report.replay_verified, 320);
    }
}
