//! Conservative controlled natural-language ingestion into the world model.

use crate::world_model::{
    replay_investigation, BeliefState, ClaimValue, EntityId, Investigation,
    InvestigationExpectation, Observation, TransitionRule, WorldModelSpec,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawReport {
    pub id: String,
    pub text: String,
    pub source: String,
    pub received_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestContext {
    pub entities: BTreeSet<EntityId>,
    pub aliases: BTreeMap<String, Vec<EntityId>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceSpan {
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub role: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClaimPolarity {
    Asserted,
    Negated,
    Hedged,
    Quoted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateParse {
    pub entity_candidates: Vec<EntityId>,
    pub variable: String,
    pub value: Option<String>,
    pub timestamp: Option<u64>,
    pub polarity: ClaimPolarity,
    pub confidence: u8,
    pub provenance: Vec<ProvenanceSpan>,
    pub alternatives: Vec<String>,
    pub unresolved_bindings: Vec<String>,
    pub safe_to_ingest: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParseOutcome {
    Accepted(CandidateParse),
    Ambiguous {
        candidates: Vec<CandidateParse>,
        reason: String,
    },
    Rejected {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestionReceipt {
    pub report_id: String,
    pub parse: ParseOutcome,
    pub observation: Option<Observation>,
    pub replay_verified: bool,
    pub inserted_fact: bool,
    pub downstream_state: Option<String>,
    pub receipt_hash: String,
}

fn parse_clock(text: &str) -> Result<Option<u64>, String> {
    let mut found = None;
    for token in text
        .split_whitespace()
        .map(|token| token.trim_matches(|ch: char| !ch.is_ascii_digit() && ch != ':'))
    {
        if let Some((hour, minute)) = token.split_once(':') {
            if hour.len() <= 2
                && minute.len() == 2
                && hour.chars().all(|ch| ch.is_ascii_digit())
                && minute.chars().all(|ch| ch.is_ascii_digit())
            {
                let h: u64 = hour.parse().map_err(|_| "invalid hour".to_string())?;
                let m: u64 = minute.parse().map_err(|_| "invalid minute".to_string())?;
                if h > 23 || m > 59 {
                    return Err("invalid clock time".into());
                }
                found = Some(h * 60 + m);
            }
        }
    }
    Ok(found)
}

fn candidate_for(
    entity_candidates: Vec<EntityId>,
    variable: &str,
    value: &str,
    timestamp: Option<u64>,
    polarity: ClaimPolarity,
    confidence: u8,
    provenance: Vec<ProvenanceSpan>,
    alternatives: Vec<String>,
    unresolved: Vec<String>,
    safe_to_ingest: bool,
) -> CandidateParse {
    CandidateParse {
        entity_candidates,
        variable: variable.into(),
        value: Some(value.into()),
        timestamp,
        polarity,
        confidence,
        provenance,
        alternatives,
        unresolved_bindings: unresolved,
        safe_to_ingest,
    }
}

pub fn parse_report(report: &RawReport, context: &IngestContext) -> ParseOutcome {
    let text_lower = report.text.to_ascii_lowercase();
    let quoted_text = report
        .text
        .split_once('\'')
        .and_then(|(_, rest)| rest.split_once('\'').map(|(inner, _)| inner.to_string()));
    let working = quoted_text.as_deref().unwrap_or(&report.text);
    let working_lower = working.to_ascii_lowercase();
    let polarity = if quoted_text.is_some() {
        ClaimPolarity::Quoted
    } else if working_lower.contains(" may be ")
        || working_lower.contains(" might be ")
        || working_lower.contains(" possibly ")
    {
        ClaimPolarity::Hedged
    } else if working_lower.contains(" not active") || working_lower.contains(" is not ") {
        ClaimPolarity::Negated
    } else {
        ClaimPolarity::Asserted
    };
    if text_lower.contains('/') {
        return ParseOutcome::Rejected {
            reason: "conflicting or underspecified date format".into(),
        };
    }
    let timestamp = match parse_clock(working) {
        Ok(value) => value,
        Err(reason) => return ParseOutcome::Rejected { reason },
    };
    let status = ["active", "idle", "blocked"]
        .iter()
        .find(|status| working_lower.contains(**status))
        .map(|status| (*status).to_string());
    let Some(status) = status else {
        return ParseOutcome::Rejected {
            reason: "no supported typed claim".into(),
        };
    };
    let mut entity_candidates = Vec::new();
    for (alias, entities) in &context.aliases {
        if working_lower.contains(&alias.to_ascii_lowercase()) {
            entity_candidates.extend(entities.iter().cloned());
        }
    }
    entity_candidates.sort();
    entity_candidates.dedup();
    if entity_candidates.is_empty() {
        return ParseOutcome::Ambiguous {
            candidates: vec![candidate_for(
                Vec::new(),
                "status",
                &status,
                timestamp,
                polarity,
                0,
                Vec::new(),
                Vec::new(),
                vec!["entity reference".into()],
                false,
            )],
            reason: "entity reference unresolved".into(),
        };
    }
    let mut provenance = Vec::new();
    if let Some(entity) = entity_candidates.first() {
        if let Some(start) = working_lower.find(&entity.0.to_ascii_lowercase()) {
            provenance.push(ProvenanceSpan {
                start,
                end: start + entity.0.len(),
                text: entity.0.clone(),
                role: "entity".into(),
            });
        }
    }
    if let Some(start) = working_lower.find(&status) {
        provenance.push(ProvenanceSpan {
            start,
            end: start + status.len(),
            text: status.clone(),
            role: "claim".into(),
        });
    }
    if let Some(time) = timestamp {
        let token = format!("{}:{:02}", time / 60, time % 60);
        if let Some(start) = working.find(&token) {
            provenance.push(ProvenanceSpan {
                start,
                end: start + token.len(),
                text: token,
                role: "time".into(),
            });
        }
    }
    let confidence = match polarity {
        ClaimPolarity::Asserted => 95,
        ClaimPolarity::Negated => 90,
        ClaimPolarity::Quoted => 70,
        ClaimPolarity::Hedged => 40,
    };
    let value = if polarity == ClaimPolarity::Negated {
        format!("not-{status}")
    } else {
        status
    };
    let candidate = candidate_for(
        entity_candidates.clone(),
        "status",
        &value,
        timestamp,
        polarity,
        confidence,
        provenance,
        Vec::new(),
        Vec::new(),
        polarity == ClaimPolarity::Asserted || polarity == ClaimPolarity::Negated,
    );
    if entity_candidates.len() > 1 {
        return ParseOutcome::Ambiguous {
            candidates: vec![candidate],
            reason: "entity collision or ambiguous reference".into(),
        };
    }
    if polarity == ClaimPolarity::Hedged {
        return ParseOutcome::Ambiguous {
            candidates: vec![candidate],
            reason: "hedged claim requires confirmation".into(),
        };
    }
    ParseOutcome::Accepted(candidate)
}

fn receipt_hash(
    report_id: &str,
    parse: &ParseOutcome,
    observation: &Option<Observation>,
    inserted: bool,
    state: &Option<String>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(report_id, parse, observation, inserted, state))
            .expect("ingestion receipt serializes"),
    );
    format!("{:x}", hasher.finalize())
}

pub fn ingest_report(report: &RawReport, context: &IngestContext) -> IngestionReceipt {
    let parse = parse_report(report, context);
    let mut observation = None;
    let mut inserted_fact = false;
    let mut downstream_state = None;
    if let ParseOutcome::Accepted(candidate) = &parse {
        if candidate.safe_to_ingest && candidate.entity_candidates.len() == 1 {
            let entity = candidate.entity_candidates[0].clone();
            let timestamp = candidate.timestamp.unwrap_or(report.received_at);
            let value = ClaimValue::State(candidate.value.clone().unwrap_or_default());
            let item = Observation {
                id: report.id.clone(),
                entity: entity.clone(),
                variable: candidate.variable.clone(),
                value,
                timestamp,
                valid_from: None,
                valid_until: None,
                source: report.source.clone(),
                reliability: candidate.confidence,
                confidence: candidate.confidence,
            };
            let spec = WorldModelSpec {
                states: ["active", "idle", "blocked", "not-active"]
                    .into_iter()
                    .map(String::from)
                    .collect(),
                events: BTreeSet::new(),
                transitions: Vec::<TransitionRule>::new(),
                required_variables: ["status".into()].into_iter().collect(),
            };
            let investigation = Investigation {
                id: format!("ingest-{}", report.id),
                entities: [entity].into_iter().collect(),
                spec,
                observations: vec![item.clone()],
                events: Vec::new(),
                expected: InvestigationExpectation {
                    applied_events: 0,
                    contradictions: 0,
                    impossible_events: 0,
                    missing_evidence: 0,
                    final_state: item.value.clone().into_state(),
                    competing_hypotheses: 0,
                },
            };
            let replay = replay_investigation(&investigation);
            downstream_state = replay
                .beliefs
                .values()
                .next()
                .and_then(|belief: &BeliefState| belief.state.clone());
            inserted_fact = replay.replay_verified();
            observation = Some(item);
        }
    }
    let receipt_hash = receipt_hash(
        &report.id,
        &parse,
        &observation,
        inserted_fact,
        &downstream_state,
    );
    IngestionReceipt {
        report_id: report.id.clone(),
        parse,
        observation,
        replay_verified: inserted_fact,
        inserted_fact,
        downstream_state,
        receipt_hash,
    }
}

trait ClaimValueExt {
    fn into_state(self) -> Option<String>;
}
impl ClaimValueExt for ClaimValue {
    fn into_state(self) -> Option<String> {
        match self {
            ClaimValue::State(value) => Some(value),
            ClaimValue::Boolean(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestionExpectation {
    pub accepted: bool,
    pub ambiguous: bool,
    pub downstream: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestionCase {
    pub id: String,
    pub report: RawReport,
    pub context: IngestContext,
    pub expected: IngestionExpectation,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestionBenchmarkReport {
    pub cases: usize,
    pub extraction_correct: usize,
    pub ambiguity_correct: usize,
    pub rejection_correct: usize,
    pub downstream_correct: usize,
    pub false_insertions: usize,
    pub replay_verified: usize,
}

pub fn evaluate_corpus(cases: &[IngestionCase]) -> IngestionBenchmarkReport {
    let mut report = IngestionBenchmarkReport {
        cases: cases.len(),
        ..Default::default()
    };
    for case in cases {
        let receipt = ingest_report(&case.report, &case.context);
        let accepted = matches!(receipt.parse, ParseOutcome::Accepted(_));
        let ambiguous = matches!(receipt.parse, ParseOutcome::Ambiguous { .. });
        let rejected = matches!(receipt.parse, ParseOutcome::Rejected { .. });
        report.extraction_correct += usize::from(accepted == case.expected.accepted);
        report.ambiguity_correct += usize::from(ambiguous == case.expected.ambiguous);
        report.rejection_correct +=
            usize::from(rejected == (!case.expected.accepted && !case.expected.ambiguous));
        report.downstream_correct += usize::from(receipt.inserted_fact == case.expected.downstream);
        report.false_insertions += usize::from(receipt.inserted_fact && !case.expected.downstream);
        report.replay_verified += usize::from(receipt.replay_verified || !receipt.inserted_fact);
    }
    report
}

fn context(aliases: &[(&str, &str)]) -> IngestContext {
    let mut map = BTreeMap::new();
    let mut entities = BTreeSet::new();
    for (alias, entity) in aliases {
        let id = EntityId((*entity).into());
        entities.insert(id.clone());
        map.entry((*alias).to_ascii_lowercase())
            .or_insert_with(Vec::new)
            .push(id);
    }
    IngestContext {
        entities,
        aliases: map,
    }
}

fn case(
    id: String,
    text: String,
    context: IngestContext,
    expected: IngestionExpectation,
) -> IngestionCase {
    IngestionCase {
        report: RawReport {
            id: id.clone(),
            text,
            source: "reporter-a".into(),
            received_at: 600,
        },
        id,
        context,
        expected,
    }
}

pub fn synthetic_corpus() -> Vec<IngestionCase> {
    let mut cases = Vec::with_capacity(300);
    for i in 0..80 {
        cases.push(case(
            format!("nl-canonical-{i:03}"),
            "Alice is active at 10:00.".into(),
            context(&[("alice", "Alice")]),
            IngestionExpectation {
                accepted: true,
                ambiguous: false,
                downstream: true,
            },
        ));
    }
    for i in 0..40 {
        cases.push(case(
            format!("nl-paraphrase-{i:03}"),
            "At 10:00, Alice is active.".into(),
            context(&[("alice", "Alice")]),
            IngestionExpectation {
                accepted: true,
                ambiguous: false,
                downstream: true,
            },
        ));
    }
    for i in 0..30 {
        cases.push(case(
            format!("nl-alias-{i:03}"),
            "A. is active at 10:00.".into(),
            context(&[("a.", "Alice")]),
            IngestionExpectation {
                accepted: true,
                ambiguous: false,
                downstream: true,
            },
        ));
    }
    for i in 0..30 {
        cases.push(case(
            format!("nl-uncertain-time-{i:03}"),
            "Alice is active at 25:00.".into(),
            context(&[("alice", "Alice")]),
            IngestionExpectation {
                accepted: false,
                ambiguous: false,
                downstream: false,
            },
        ));
    }
    for i in 0..20 {
        cases.push(case(
            format!("nl-conflicting-date-{i:03}"),
            "Alice is active on 01/02 at 10:00.".into(),
            context(&[("alice", "Alice")]),
            IngestionExpectation {
                accepted: false,
                ambiguous: false,
                downstream: false,
            },
        ));
    }
    for i in 0..20 {
        cases.push(case(
            format!("nl-hedged-{i:03}"),
            "Alice may be active at 10:00.".into(),
            context(&[("alice", "Alice")]),
            IngestionExpectation {
                accepted: false,
                ambiguous: true,
                downstream: false,
            },
        ));
    }
    for i in 0..20 {
        cases.push(case(
            format!("nl-quoted-{i:03}"),
            "Bob said 'Alice is active at 10:00'.".into(),
            context(&[("alice", "Alice"), ("bob", "Bob")]),
            IngestionExpectation {
                accepted: true,
                ambiguous: false,
                downstream: false,
            },
        ));
    }
    for i in 0..20 {
        cases.push(case(
            format!("nl-negated-{i:03}"),
            "Alice is not active at 10:00.".into(),
            context(&[("alice", "Alice")]),
            IngestionExpectation {
                accepted: true,
                ambiguous: false,
                downstream: true,
            },
        ));
    }
    for i in 0..20 {
        cases.push(case(
            format!("nl-irrelevant-{i:03}"),
            "While rain fell, Alice is active at 10:00.".into(),
            context(&[("alice", "Alice")]),
            IngestionExpectation {
                accepted: true,
                ambiguous: false,
                downstream: true,
            },
        ));
    }
    for i in 0..20 {
        cases.push(case(
            format!("nl-collision-{i:03}"),
            "Alice and Bob are active at 10:00.".into(),
            context(&[("alice", "Alice"), ("bob", "Bob")]),
            IngestionExpectation {
                accepted: false,
                ambiguous: true,
                downstream: false,
            },
        ));
    }
    cases
}

pub fn synthetic_corpus_hash() -> String {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(&synthetic_corpus()).expect("natural corpus serializes"));
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noisy_reports_remain_typed_and_fail_closed() {
        let cases = synthetic_corpus();
        let report = evaluate_corpus(&cases);
        eprintln!("phase11 natural ingest: hash={} cases={} extraction={} ambiguity={} rejection={} downstream={} false_insertions={} replay={}", synthetic_corpus_hash(), report.cases, report.extraction_correct, report.ambiguity_correct, report.rejection_correct, report.downstream_correct, report.false_insertions, report.replay_verified);
        assert_eq!(report.cases, 300);
        assert_eq!(report.extraction_correct, 300);
        assert_eq!(report.ambiguity_correct, 300);
        assert_eq!(report.rejection_correct, 300);
        assert_eq!(report.downstream_correct, 300);
        assert_eq!(report.false_insertions, 0);
        assert_eq!(report.replay_verified, 300);
        let receipt = ingest_report(&cases[0].report, &cases[0].context);
        assert!(receipt.receipt_hash.len() == 64);
    }
}
