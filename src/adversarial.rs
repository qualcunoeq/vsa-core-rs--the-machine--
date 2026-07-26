//! Correlation-aware and causally constrained evidence analysis.

use crate::epistemic::{EpistemicInvestigation, EvidenceRecord, Hypothesis, HypothesisId, SourceFailureMode, MIN_DECISIVE_EVIDENCE_SCORE};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdversarialOutcome {
    BestKnownHypothesis { hypothesis_id: HypothesisId },
    MultiplePlausibleHypotheses { hypothesis_ids: Vec<HypothesisId> },
    NovelHypothesisNeeded,
    InsufficientEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdversarialAnalysis {
    pub outcome: AdversarialOutcome,
    pub plausible_hypotheses: Vec<HypothesisId>,
    pub unexplained_observations: Vec<String>,
    pub source_failures: BTreeMap<String, SourceFailureMode>,
    pub duplicate_observations: usize,
    pub causal_mismatches: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdversarialReplayReceipt {
    pub investigation_id: String,
    pub outcome: AdversarialOutcome,
    pub plausible_hypotheses: Vec<HypothesisId>,
    pub unexplained_observations: Vec<String>,
    pub source_failures: BTreeMap<String, SourceFailureMode>,
    pub duplicate_observations: usize,
    pub causal_mismatches: usize,
    pub replay_hash: String,
}

impl AdversarialReplayReceipt {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == receipt_hash(&self.outcome, &self.plausible_hypotheses, &self.unexplained_observations, &self.source_failures, self.duplicate_observations, self.causal_mismatches)
    }
}

fn receipt_hash(outcome: &AdversarialOutcome, plausible: &[HypothesisId], unexplained: &[String], failures: &BTreeMap<String, SourceFailureMode>, duplicates: usize, causal_mismatches: usize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(&(outcome, plausible, unexplained, failures, duplicates, causal_mismatches)).expect("adversarial receipt serializes"));
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdversarialInvestigation {
    pub id: String,
    pub epistemic: EpistemicInvestigation,
    pub expected_outcome: AdversarialOutcome,
    pub expected_failure_count: usize,
    pub ground_truth: Option<HypothesisId>,
}

fn origin(record: &EvidenceRecord) -> String {
    record.correlation_group.clone().or_else(|| record.ancestry.first().cloned()).unwrap_or_else(|| record.source.clone())
}

fn path_compatible(hypothesis: &Hypothesis, query_id: &str, evidence: &EvidenceRecord) -> bool {
    let Some(expected) = hypothesis.causal_paths.get(query_id) else { return evidence.causal_path.is_empty(); };
    evidence.causal_path == *expected || evidence.causal_path.starts_with(expected)
}

fn reliable_records(investigation: &AdversarialInvestigation) -> (Vec<&EvidenceRecord>, BTreeMap<String, SourceFailureMode>, usize) {
    let as_of = investigation.epistemic.evidence.iter().map(|record| record.timestamp).max().unwrap_or(0);
    let mut failures = BTreeMap::new();
    let mut seen = BTreeSet::new();
    let mut duplicates = 0;
    let mut records = Vec::new();
    for record in &investigation.epistemic.evidence {
        if let Some(mode) = &record.failure_mode { failures.insert(record.source.clone(), mode.clone()); }
        if !record.valid_at(as_of) || u16::from(record.reliability) * u16::from(record.confidence) < MIN_DECISIVE_EVIDENCE_SCORE { continue; }
        let key = (record.query_id.clone(), origin(record), record.outcome.clone());
        if !seen.insert(key) { duplicates += 1; continue; }
        if record.failure_mode.is_none() { records.push(record); }
    }
    (records, failures, duplicates)
}

pub fn analyze(investigation: &AdversarialInvestigation) -> AdversarialAnalysis {
    let (records, source_failures, duplicate_observations) = reliable_records(investigation);
    if records.is_empty() {
        return AdversarialAnalysis { outcome: AdversarialOutcome::InsufficientEvidence, plausible_hypotheses: investigation.epistemic.hypotheses.iter().map(|h| h.id.clone()).collect(), unexplained_observations: Vec::new(), source_failures, duplicate_observations, causal_mismatches: 0 };
    }
    let mut plausible: BTreeSet<HypothesisId> = investigation.epistemic.hypotheses.iter().map(|h| h.id.clone()).collect();
    let mut by_query: BTreeMap<&str, Vec<&EvidenceRecord>> = BTreeMap::new();
    for record in &records { by_query.entry(&record.query_id).or_default().push(record); }
    let mut unexplained = Vec::new();
    let mut causal_mismatches = 0;
    for (query_id, query_records) in by_query {
        let mut support: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for record in &query_records {
            support.entry(record.outcome.clone()).or_default().insert(origin(record));
        }
        let Some(max_support) = support.values().map(BTreeSet::len).max() else { continue };
        let winners: Vec<&String> = support.iter().filter(|(_, origins)| origins.len() == max_support).map(|(outcome, _)| outcome).collect();
        if winners.len() != 1 { continue; }
        let outcome = winners[0];
        let matching: BTreeSet<HypothesisId> = investigation.epistemic.hypotheses.iter().filter(|hypothesis| plausible.contains(&hypothesis.id) && hypothesis.predictions.get(query_id) == Some(outcome) && query_records.iter().any(|record| record.outcome == *outcome && path_compatible(hypothesis, query_id, record))).map(|h| h.id.clone()).collect();
        let lexical = investigation.epistemic.hypotheses.iter().any(|h| plausible.contains(&h.id) && h.predictions.get(query_id) == Some(outcome));
        if !matching.is_empty() { plausible = plausible.intersection(&matching).cloned().collect(); }
        else {
            if lexical { causal_mismatches += 1; }
            unexplained.extend(query_records.iter().filter(|record| record.outcome == *outcome).map(|record| record.id.clone()));
        }
    }
    let plausible_hypotheses = plausible.into_iter().collect::<Vec<_>>();
    let outcome = if !unexplained.is_empty() { AdversarialOutcome::NovelHypothesisNeeded } else if plausible_hypotheses.len() == 1 { AdversarialOutcome::BestKnownHypothesis { hypothesis_id: plausible_hypotheses[0].clone() } } else if plausible_hypotheses.len() > 1 { AdversarialOutcome::MultiplePlausibleHypotheses { hypothesis_ids: plausible_hypotheses.clone() } } else { AdversarialOutcome::NovelHypothesisNeeded };
    AdversarialAnalysis { outcome, plausible_hypotheses, unexplained_observations: unexplained, source_failures, duplicate_observations, causal_mismatches }
}

pub fn replay(investigation: &AdversarialInvestigation) -> AdversarialReplayReceipt {
    let analysis = analyze(investigation);
    let replay_hash = receipt_hash(&analysis.outcome, &analysis.plausible_hypotheses, &analysis.unexplained_observations, &analysis.source_failures, analysis.duplicate_observations, analysis.causal_mismatches);
    AdversarialReplayReceipt { investigation_id: investigation.id.clone(), outcome: analysis.outcome, plausible_hypotheses: analysis.plausible_hypotheses, unexplained_observations: analysis.unexplained_observations, source_failures: analysis.source_failures, duplicate_observations: analysis.duplicate_observations, causal_mismatches: analysis.causal_mismatches, replay_hash }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdversarialBenchmarkReport {
    pub cases: usize,
    pub outcomes_correct: usize,
    pub misleading_resistance: usize,
    pub resistance_cases: usize,
    pub correlation_aware: usize,
    pub source_failures_identified: usize,
    pub overcounting_detected: usize,
    pub overcount_cases: usize,
    pub causal_compatibility: usize,
    pub replay_verified: usize,
}

pub fn evaluate_corpus(cases: &[AdversarialInvestigation]) -> AdversarialBenchmarkReport {
    let mut report = AdversarialBenchmarkReport { cases: cases.len(), ..Default::default() };
    for case in cases {
        let analysis = analyze(case);
        let receipt = replay(case);
        report.outcomes_correct += usize::from(analysis.outcome == case.expected_outcome);
        if let Some(truth) = &case.ground_truth { report.resistance_cases += 1; report.misleading_resistance += usize::from(analysis.plausible_hypotheses.contains(truth)); }
        let expected_correlated = case.epistemic.evidence.iter().fold(BTreeSet::new(), |mut seen, evidence| { if let Some(group) = &evidence.correlation_group { if !seen.insert((evidence.query_id.clone(), group.clone())) { return seen; } } seen }).len() < case.epistemic.evidence.iter().filter(|e| e.correlation_group.is_some()).count();
        report.correlation_aware += usize::from(expected_correlated == (analysis.duplicate_observations > 0));
        report.source_failures_identified += usize::from(analysis.source_failures.len() == case.expected_failure_count);
        if expected_correlated { report.overcount_cases += 1; report.overcounting_detected += usize::from(analysis.duplicate_observations > 0); }
        let expected_causal_mismatch = case.epistemic.evidence.iter().any(|e| e.causal_path == ["other".to_string(), "path".to_string()]);
        report.causal_compatibility += usize::from(expected_causal_mismatch == (analysis.causal_mismatches > 0));
        report.replay_verified += usize::from(receipt.replay_verified());
    }
    report
}

fn hypothesis(id: &str, outcome: &str, path: &[&str]) -> Hypothesis {
    Hypothesis { id: HypothesisId(id.into()), description: format!("hypothesis {id}"), predictions: [("q".into(), outcome.into())].into_iter().collect(), causal_paths: [("q".into(), path.iter().map(|item| (*item).into()).collect())].into_iter().collect() }
}

fn query() -> crate::epistemic::EvidenceQuery { crate::epistemic::EvidenceQuery { id: "q".into(), description: "test causal observation".into(), cost: 1 } }

fn record(id: &str, outcome: &str, source: &str, group: Option<&str>, failure: Option<SourceFailureMode>, causal_path: &[&str], reliability: u8) -> EvidenceRecord {
    EvidenceRecord { id: id.into(), query_id: "q".into(), outcome: outcome.into(), timestamp: 1, valid_until: None, source: source.into(), reliability, confidence: 90, ancestry: group.map(|item| vec![item.into()]).unwrap_or_default(), correlation_group: group.map(str::to_string), failure_mode: failure, causal_path: causal_path.iter().map(|item| (*item).into()).collect() }
}

fn case(id: String, hypotheses: Vec<Hypothesis>, evidence: Vec<EvidenceRecord>, expected_outcome: AdversarialOutcome, failures: usize) -> AdversarialInvestigation {
    let ground_truth = match &expected_outcome { AdversarialOutcome::BestKnownHypothesis { hypothesis_id } => Some(hypothesis_id.clone()), _ => None };
    let base = EpistemicInvestigation { id: id.clone(), hypotheses, queries: vec![query()], evidence, ground_truth: ground_truth.clone(), expected_recommendation: crate::epistemic::Recommendation::NoDiscriminatingEvidence };
    AdversarialInvestigation { id, epistemic: base, expected_outcome, expected_failure_count: failures, ground_truth }
}

pub fn synthetic_corpus() -> Vec<AdversarialInvestigation> {
    let mut cases = Vec::with_capacity(300);
    let h_a = || hypothesis("h-a", "correct", &["event", "sensor"]);
    let h_b = || hypothesis("h-b", "wrong", &["event", "sensor"]);
    for i in 0..40 { cases.push(case(format!("adv-duplicate-{i:03}"), vec![h_a(), h_b()], vec![record("correct-a", "correct", "independent-a", Some("a"), None, &["event", "sensor"], 30), record("correct-b", "correct", "independent-b", Some("b"), None, &["event", "sensor"], 30), record("wrong-1", "wrong", "camera", Some("camera-root"), None, &["event", "sensor"], 100), record("wrong-2", "wrong", "log", Some("camera-root"), None, &["event", "sensor"], 100)], AdversarialOutcome::BestKnownHypothesis { hypothesis_id: HypothesisId("h-a".into()) }, 0)); }
    for i in 0..40 { cases.push(case(format!("adv-correlated-{i:03}"), vec![h_a(), h_b()], vec![record("right-a", "correct", "sensor-a", Some("a"), None, &["event", "sensor"], 25), record("right-b", "correct", "sensor-b", Some("b"), None, &["event", "sensor"], 25), record("wrong-a", "wrong", "camera", Some("camera"), None, &["event", "sensor"], 100), record("wrong-b", "wrong", "report", Some("camera"), None, &["event", "sensor"], 100)], AdversarialOutcome::BestKnownHypothesis { hypothesis_id: HypothesisId("h-a".into()) }, 0)); }
    for i in 0..30 { cases.push(case(format!("adv-copied-{i:03}"), vec![h_a(), h_b()], vec![record("right", "correct", "independent", Some("independent"), None, &["event", "sensor"], 25), record("copy-a", "wrong", "testimony-a", Some("footage"), Some(SourceFailureMode::CopiedReport), &["event", "sensor"], 100), record("copy-b", "wrong", "testimony-b", Some("footage"), Some(SourceFailureMode::CopiedReport), &["event", "sensor"], 100)], AdversarialOutcome::BestKnownHypothesis { hypothesis_id: HypothesisId("h-a".into()) }, 2)); }
    for (offset, mode) in [(0, SourceFailureMode::ClockDrift), (30, SourceFailureMode::IdentityConfusion), (60, SourceFailureMode::StaleCache)] {
        for i in 0..30 { let id = offset + i; let stale = matches!(mode, SourceFailureMode::StaleCache); let mut bad = record("bad", "wrong", "faulty", Some("faulty"), Some(mode.clone()), &["event", "sensor"], 100); if stale { bad.valid_until = Some(0); } cases.push(case(format!("adv-failure-{id:03}"), vec![h_a(), h_b()], vec![record("right", "correct", "independent", Some("independent"), None, &["event", "sensor"], 25), bad], AdversarialOutcome::BestKnownHypothesis { hypothesis_id: HypothesisId("h-a".into()) }, 1)); }
    }
    for i in 0..30 { cases.push(case(format!("adv-adversarial-{i:03}"), vec![h_a(), h_b()], vec![record("right", "correct", "independent", Some("independent"), None, &["event", "sensor"], 25), record("attack-a", "wrong", "attacker", Some("attacker"), Some(SourceFailureMode::AdversarialFabrication), &["event", "sensor"], 100), record("attack-b", "wrong", "attacker-log", Some("attacker"), Some(SourceFailureMode::AdversarialFabrication), &["event", "sensor"], 100)], AdversarialOutcome::BestKnownHypothesis { hypothesis_id: HypothesisId("h-a".into()) }, 2)); }
    for i in 0..30 { cases.push(case(format!("adv-omitted-{i:03}"), vec![h_a(), h_b()], vec![record("residual", "novel", "independent", Some("independent"), None, &["event", "sensor"], 90)], AdversarialOutcome::NovelHypothesisNeeded, 0)); }
    for i in 0..20 { cases.push(case(format!("adv-causal-{i:03}"), vec![h_a(), h_b()], vec![record("lexical", "correct", "sensor", Some("sensor"), None, &["other", "path"], 90)], AdversarialOutcome::NovelHypothesisNeeded, 0)); }
    for i in 0..20 { cases.push(case(format!("adv-unresolved-{i:03}"), vec![h_a(), h_b()], vec![record("tie-a", "correct", "a", Some("a"), None, &["event", "sensor"], 50), record("tie-b", "wrong", "b", Some("b"), None, &["event", "sensor"], 50)], AdversarialOutcome::MultiplePlausibleHypotheses { hypothesis_ids: vec![HypothesisId("h-a".into()), HypothesisId("h-b".into())] }, 0)); }
    cases
}

pub fn synthetic_corpus_hash() -> String {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(&synthetic_corpus()).expect("adversarial corpus serializes"));
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adversarial_evidence_does_not_triple_count_one_origin() {
        let cases = synthetic_corpus();
        let report = evaluate_corpus(&cases);
        eprintln!("phase8 adversarial: hash={} cases={} outcomes={} resistance={}/{} correlation={} failures={} overcount={}/{} causal={} replay={}", synthetic_corpus_hash(), report.cases, report.outcomes_correct, report.misleading_resistance, report.resistance_cases, report.correlation_aware, report.source_failures_identified, report.overcounting_detected, report.overcount_cases, report.causal_compatibility, report.replay_verified);
        assert_eq!(report.cases, 300);
        assert_eq!(report.outcomes_correct, 300);
        assert_eq!(report.resistance_cases, 230);
        assert_eq!(report.misleading_resistance, 230);
        assert_eq!(report.correlation_aware, 300);
        assert_eq!(report.source_failures_identified, 300);
        assert_eq!(report.overcount_cases, 140);
        assert_eq!(report.overcounting_detected, 140);
        assert_eq!(report.causal_compatibility, 300);
        assert_eq!(report.replay_verified, 300);
        let mut tampered = replay(&cases[0]);
        tampered.duplicate_observations += 1;
        assert!(!tampered.replay_verified());
    }
}
