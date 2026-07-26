//! Bounded hypothesis testing and information-seeking over the world ledger.
//!
//! This layer is deliberately diagnostic: it produces predictions, compares
//! candidate observations, and recommends evidence to collect. It never
//! authorizes an action or mutates the world-model registry.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HypothesisId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hypothesis {
    pub id: HypothesisId,
    pub description: String,
    /// Query id -> predicted outcome.
    pub predictions: BTreeMap<String, String>,
    /// Query id -> causal path that should produce the prediction.
    pub causal_paths: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceFailureMode {
    ClockDrift,
    IdentityConfusion,
    CopiedReport,
    StaleCache,
    SelectiveOmission,
    AdversarialFabrication,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub id: String,
    pub query_id: String,
    pub outcome: String,
    pub timestamp: u64,
    pub valid_until: Option<u64>,
    pub source: String,
    pub reliability: u8,
    pub confidence: u8,
    pub ancestry: Vec<String>,
    pub correlation_group: Option<String>,
    pub failure_mode: Option<SourceFailureMode>,
    pub causal_path: Vec<String>,
}

impl EvidenceRecord {
    pub fn valid_at(&self, timestamp: u64) -> bool {
        self.valid_until.is_none_or(|end| timestamp <= end)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceQuery {
    pub id: String,
    pub description: String,
    pub cost: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Recommendation {
    Recommend { query_id: String },
    Ambiguous { query_ids: Vec<String> },
    NoDiscriminatingEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InformationGain {
    /// Gain is represented exactly as numerator / denominator.
    pub numerator: u64,
    pub denominator: u64,
}

impl InformationGain {
    pub(crate) fn ratio_cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.numerator * other.denominator).cmp(&(other.numerator * self.denominator))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryAssessment {
    pub query_id: String,
    pub information_gain: InformationGain,
    pub partitions: BTreeMap<String, usize>,
    pub discriminating: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpistemicAnalysis {
    pub plausible_hypotheses: Vec<HypothesisId>,
    pub predictions: BTreeMap<HypothesisId, BTreeMap<String, String>>,
    pub shared_predictions: BTreeMap<String, String>,
    pub assessments: Vec<QueryAssessment>,
    pub recommendation: Recommendation,
    pub unsupported_prediction_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeliefUpdate {
    pub evidence_id: String,
    pub retained: Vec<HypothesisId>,
    pub discarded: Vec<HypothesisId>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpistemicReplayReceipt {
    pub investigation_id: String,
    pub updates: Vec<BeliefUpdate>,
    pub final_plausible: Vec<HypothesisId>,
    pub replay_hash: String,
}

impl EpistemicReplayReceipt {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == receipt_hash(&self.updates, &self.final_plausible)
            && self.updates.iter().all(|update| !update.evidence_id.is_empty())
            && !self.final_plausible.is_empty()
    }
}

fn receipt_hash(updates: &[BeliefUpdate], final_plausible: &[HypothesisId]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(&(updates, final_plausible)).expect("epistemic receipt serializes"));
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpistemicInvestigation {
    pub id: String,
    pub hypotheses: Vec<Hypothesis>,
    pub queries: Vec<EvidenceQuery>,
    pub evidence: Vec<EvidenceRecord>,
    pub ground_truth: Option<HypothesisId>,
    pub expected_recommendation: Recommendation,
}

fn score(record: &EvidenceRecord) -> u16 {
    u16::from(record.reliability) * u16::from(record.confidence)
}

pub(crate) const MIN_DECISIVE_EVIDENCE_SCORE: u16 = 2_000;

fn usable_evidence(investigation: &EpistemicInvestigation) -> Vec<&EvidenceRecord> {
    let as_of = investigation.evidence.iter().map(|record| record.timestamp).max().unwrap_or(0);
    investigation.evidence.iter().filter(|record| record.valid_at(as_of) && score(record) >= MIN_DECISIVE_EVIDENCE_SCORE).collect()
}

fn plausible_after_evidence(investigation: &EpistemicInvestigation) -> BTreeSet<HypothesisId> {
    let mut plausible: BTreeSet<HypothesisId> = investigation.hypotheses.iter().map(|h| h.id.clone()).collect();
    let mut by_query: BTreeMap<&str, Vec<&EvidenceRecord>> = BTreeMap::new();
    for record in usable_evidence(investigation) {
        by_query.entry(&record.query_id).or_default().push(record);
    }
    for (query_id, records) in by_query {
        let Some(max_score) = records.iter().map(|record| score(record)).max() else { continue };
        let winners: BTreeSet<&str> = records.iter().filter(|record| score(record) == max_score).map(|record| record.outcome.as_str()).collect();
        if winners.len() != 1 { continue; }
        let outcome = *winners.iter().next().expect("one winning outcome");
        let matching: BTreeSet<HypothesisId> = investigation.hypotheses.iter().filter(|hypothesis| hypothesis.predictions.get(query_id).is_some_and(|prediction| prediction == outcome)).map(|hypothesis| hypothesis.id.clone()).collect();
        if !matching.is_empty() { plausible = plausible.intersection(&matching).cloned().collect(); }
    }
    plausible
}

pub fn analyze(investigation: &EpistemicInvestigation) -> EpistemicAnalysis {
    let plausible: Vec<HypothesisId> = plausible_after_evidence(investigation).into_iter().collect();
    let predictions = investigation.hypotheses.iter().filter(|hypothesis| plausible.contains(&hypothesis.id)).map(|hypothesis| (hypothesis.id.clone(), hypothesis.predictions.clone())).collect::<BTreeMap<_, _>>();
    let mut shared_predictions = BTreeMap::new();
    let mut assessments = Vec::new();
    let mut unsupported = 0;
    for query in &investigation.queries {
        let outcomes: Vec<&str> = plausible.iter().filter_map(|id| predictions.get(id)?.get(&query.id).map(String::as_str)).collect();
        if outcomes.len() != plausible.len() { unsupported += 1; continue; }
        let mut partitions = BTreeMap::new();
        for outcome in outcomes { *partitions.entry(outcome.to_string()).or_insert(0) += 1; }
        if partitions.len() == 1 { shared_predictions.insert(query.id.clone(), partitions.keys().next().expect("one outcome").clone()); }
        let total = plausible.len() as u64;
        let sum_squares = partitions.values().map(|size| (*size as u64) * (*size as u64)).sum::<u64>();
        let gain = InformationGain { numerator: total * total - sum_squares, denominator: total.max(1) };
        assessments.push(QueryAssessment { query_id: query.id.clone(), information_gain: gain, partitions: partitions.clone(), discriminating: partitions.len() > 1 });
    }
    let discriminating: Vec<&QueryAssessment> = assessments.iter().filter(|assessment| assessment.discriminating).collect();
    let recommendation = if discriminating.is_empty() { Recommendation::NoDiscriminatingEvidence } else {
        let best_gain = discriminating.iter().map(|assessment| &assessment.information_gain).max_by(|a, b| a.ratio_cmp(b)).expect("discriminating query");
        let best: Vec<String> = discriminating.iter().filter(|assessment| assessment.information_gain.ratio_cmp(best_gain) == std::cmp::Ordering::Equal).map(|assessment| assessment.query_id.clone()).collect();
        if best.len() == 1 { Recommendation::Recommend { query_id: best[0].clone() } } else { Recommendation::Ambiguous { query_ids: best } }
    };
    EpistemicAnalysis { plausible_hypotheses: plausible, predictions, shared_predictions, assessments, recommendation, unsupported_prediction_count: unsupported }
}

pub fn replay_beliefs(investigation: &EpistemicInvestigation) -> EpistemicReplayReceipt {
    let mut plausible: BTreeSet<HypothesisId> = investigation.hypotheses.iter().map(|h| h.id.clone()).collect();
    let as_of = investigation.evidence.iter().map(|record| record.timestamp).max().unwrap_or(0);
    let mut by_query: BTreeMap<String, Vec<&EvidenceRecord>> = BTreeMap::new();
    for record in investigation.evidence.iter().filter(|record| record.valid_at(as_of) && score(record) >= MIN_DECISIVE_EVIDENCE_SCORE) {
        by_query.entry(record.query_id.clone()).or_default().push(record);
    }
    let mut updates = Vec::new();
    for (query_id, records) in by_query {
        let Some(max_score) = records.iter().map(|record| score(record)).max() else { continue };
        let winners: BTreeSet<&str> = records.iter().filter(|record| score(record) == max_score).map(|record| record.outcome.as_str()).collect();
        if winners.len() != 1 { continue; }
        let outcome = *winners.iter().next().expect("one winning outcome");
        let matching: BTreeSet<HypothesisId> = investigation.hypotheses.iter().filter(|hypothesis| plausible.contains(&hypothesis.id) && hypothesis.predictions.get(&query_id).is_some_and(|prediction| prediction == outcome)).map(|hypothesis| hypothesis.id.clone()).collect();
        if matching.is_empty() { continue; }
        let discarded = plausible.difference(&matching).cloned().collect::<Vec<_>>();
        plausible = matching;
        let evidence_id = records.iter().find(|record| record.outcome == outcome && score(record) == max_score).expect("winning evidence").id.clone();
        updates.push(BeliefUpdate { evidence_id, retained: plausible.iter().cloned().collect(), discarded, reason: "highest-confidence compatible evidence".into() });
    }
    let final_plausible = plausible.into_iter().collect::<Vec<_>>();
    let replay_hash = receipt_hash(&updates, &final_plausible);
    EpistemicReplayReceipt { investigation_id: investigation.id.clone(), updates, final_plausible, replay_hash }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpistemicBenchmarkReport {
    pub cases: usize,
    pub ranking_correct: usize,
    pub prediction_correct: usize,
    pub recommendation_correct: usize,
    pub ambiguity_preserved: usize,
    pub unsupported_hypotheses: usize,
    pub belief_updates_correct: usize,
    pub calibration_correct: usize,
    pub replay_verified: usize,
}

pub fn evaluate_corpus(cases: &[EpistemicInvestigation]) -> EpistemicBenchmarkReport {
    let mut report = EpistemicBenchmarkReport { cases: cases.len(), ..Default::default() };
    for case in cases {
        let analysis = analyze(case);
        let receipt = replay_beliefs(case);
        report.ranking_correct += usize::from(case.ground_truth.as_ref().is_none_or(|truth| analysis.plausible_hypotheses.contains(truth)));
        report.prediction_correct += usize::from(case.ground_truth.as_ref().is_none_or(|truth| analysis.predictions.contains_key(truth)));
        report.recommendation_correct += usize::from(analysis.recommendation == case.expected_recommendation);
        report.ambiguity_preserved += usize::from(matches!(case.expected_recommendation, Recommendation::Ambiguous { .. }) && matches!(analysis.recommendation, Recommendation::Ambiguous { .. }));
        report.unsupported_hypotheses += analysis.unsupported_prediction_count;
        report.belief_updates_correct += usize::from(case.ground_truth.as_ref().is_none_or(|truth| receipt.final_plausible.contains(truth)));
        report.calibration_correct += usize::from(case.ground_truth.as_ref().map_or(analysis.plausible_hypotheses.len() > 1, |truth| analysis.plausible_hypotheses.contains(truth)));
        report.replay_verified += usize::from(receipt.replay_verified());
    }
    report
}

fn hypothesis(id: &str, predictions: &[(&str, &str)]) -> Hypothesis {
    Hypothesis { id: HypothesisId(id.into()), description: format!("hypothesis {id}"), predictions: predictions.iter().map(|(query, outcome)| ((*query).into(), (*outcome).into())).collect(), causal_paths: BTreeMap::new() }
}

fn query(id: &str) -> EvidenceQuery { EvidenceQuery { id: id.into(), description: format!("observe {id}"), cost: 1 } }

fn evidence(id: &str, query_id: &str, outcome: &str, timestamp: u64, reliability: u8) -> EvidenceRecord {
    EvidenceRecord { id: id.into(), query_id: query_id.into(), outcome: outcome.into(), timestamp, valid_until: None, source: format!("source-{id}"), reliability, confidence: 90, ancestry: Vec::new(), correlation_group: None, failure_mode: None, causal_path: Vec::new() }
}

pub fn synthetic_corpus() -> Vec<EpistemicInvestigation> {
    let mut cases = Vec::with_capacity(300);
    for index in 0..120 {
        let hypotheses = vec![hypothesis("h-a", &[("q-best", "a"), ("q-shared", "same")]), hypothesis("h-b", &[("q-best", "b"), ("q-shared", "same")]), hypothesis("h-c", &[("q-best", "c"), ("q-shared", "same")])];
        cases.push(EpistemicInvestigation { id: format!("epistemic-clear-{index:03}"), hypotheses, queries: vec![query("q-best"), query("q-shared")], evidence: vec![], ground_truth: Some(HypothesisId("h-a".into())), expected_recommendation: Recommendation::Recommend { query_id: "q-best".into() } });
    }
    for index in 0..60 {
        let hypotheses = vec![hypothesis("h-a", &[("q-a", "yes"), ("q-b", "left")]), hypothesis("h-b", &[("q-a", "no"), ("q-b", "right")]), hypothesis("h-c", &[("q-a", "yes"), ("q-b", "right")])];
        cases.push(EpistemicInvestigation { id: format!("epistemic-redundant-{index:03}"), hypotheses, queries: vec![query("q-a"), query("q-b")], evidence: vec![evidence("prior", "q-a", "yes", 1, 40)], ground_truth: Some(HypothesisId("h-a".into())), expected_recommendation: Recommendation::Recommend { query_id: "q-b".into() } });
    }
    for index in 0..50 {
        let hypotheses = vec![hypothesis("h-a", &[("q-main", "red"), ("q-check", "near")]), hypothesis("h-b", &[("q-main", "blue"), ("q-check", "far")]), hypothesis("h-c", &[("q-main", "green"), ("q-check", "far")])];
        cases.push(EpistemicInvestigation { id: format!("epistemic-misleading-{index:03}"), hypotheses, queries: vec![query("q-main"), query("q-check")], evidence: vec![evidence("weak", "q-main", "blue", 1, 10)], ground_truth: Some(HypothesisId("h-a".into())), expected_recommendation: Recommendation::Recommend { query_id: "q-main".into() } });
    }
    for index in 0..40 {
        let hypotheses = vec![hypothesis("h-a", &[("q-delay", "now"), ("q-check", "one")]), hypothesis("h-b", &[("q-delay", "later"), ("q-check", "two")]), hypothesis("h-c", &[("q-delay", "never"), ("q-check", "two")])];
        cases.push(EpistemicInvestigation { id: format!("epistemic-stale-{index:03}"), hypotheses, queries: vec![query("q-delay"), query("q-check")], evidence: vec![EvidenceRecord { valid_until: Some(0), ..evidence("stale", "q-delay", "later", 1, 100) }], ground_truth: Some(HypothesisId("h-a".into())), expected_recommendation: Recommendation::Recommend { query_id: "q-delay".into() } });
    }
    for index in 0..30 {
        let hypotheses = vec![hypothesis("h-a", &[("q-tie", "x"), ("q-tie-2", "left")]), hypothesis("h-b", &[("q-tie", "y"), ("q-tie-2", "right")]), hypothesis("h-c", &[("q-tie", "x"), ("q-tie-2", "right")])];
        cases.push(EpistemicInvestigation { id: format!("epistemic-unresolved-{index:03}"), hypotheses, queries: vec![query("q-tie"), query("q-tie-2")], evidence: vec![], ground_truth: None, expected_recommendation: Recommendation::Ambiguous { query_ids: vec!["q-tie".into(), "q-tie-2".into()] } });
    }
    cases
}

pub fn synthetic_corpus_hash() -> String {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(&synthetic_corpus()).expect("epistemic corpus serializes"));
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_investigator_preserves_uncertainty_and_replays() {
        let cases = synthetic_corpus();
        let report = evaluate_corpus(&cases);
        eprintln!("phase6 epistemic: hash={} cases={} ranking={} predictions={} recommendations={} ambiguity={} updates={} calibration={} replay={}", synthetic_corpus_hash(), report.cases, report.ranking_correct, report.prediction_correct, report.recommendation_correct, report.ambiguity_preserved, report.belief_updates_correct, report.calibration_correct, report.replay_verified);
        assert_eq!(report.cases, 300);
        assert_eq!(report.ranking_correct, 300);
        assert_eq!(report.prediction_correct, 300);
        assert_eq!(report.recommendation_correct, 300);
        assert_eq!(report.belief_updates_correct, 300);
        assert_eq!(report.calibration_correct, 300);
        assert_eq!(report.replay_verified, 300);
        let receipt = replay_beliefs(&cases[0]);
        assert!(receipt.replay_verified());
        let mut tampered = receipt.clone();
        tampered.final_plausible.clear();
        assert!(!tampered.replay_verified());
    }
}
