//! Open-set hypothesis management over the bounded epistemic investigator.
//!
//! This layer detects when the active hypothesis set cannot explain reliable
//! evidence and emits a falsifiable proposal. Proposals are diagnostic only;
//! they are never inserted into the active belief set automatically.

use crate::epistemic::{analyze, EpistemicAnalysis, EpistemicInvestigation, EvidenceRecord, Hypothesis, HypothesisId, InformationGain, Recommendation, MIN_DECISIVE_EVIDENCE_SCORE};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpenSetOutcome {
    BestKnownHypothesis { hypothesis_id: HypothesisId },
    MultiplePlausibleHypotheses { hypothesis_ids: Vec<HypothesisId> },
    NoAdequateHypothesis,
    NovelHypothesisNeeded,
    InsufficientEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HypothesisProposal {
    pub proposal_id: String,
    pub unexplained_observations: Vec<String>,
    pub minimum_latent_cause: String,
    pub predictions: BTreeMap<String, String>,
    pub overlap_hypotheses: Vec<HypothesisId>,
    pub assumptions: Vec<String>,
    pub falsification_conditions: Vec<String>,
    pub expected_information_gain: InformationGain,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenSetAnalysis {
    pub epistemic: EpistemicAnalysis,
    pub outcome: OpenSetOutcome,
    pub unexplained_observations: Vec<String>,
    pub proposal: Option<HypothesisProposal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenSetReplayReceipt {
    pub investigation_id: String,
    pub outcome: OpenSetOutcome,
    pub unexplained_observations: Vec<String>,
    pub proposal_id: Option<String>,
    pub replay_hash: String,
}

impl OpenSetReplayReceipt {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == open_set_hash(&self.outcome, &self.unexplained_observations, &self.proposal_id)
    }
}

fn open_set_hash(outcome: &OpenSetOutcome, unexplained: &[String], proposal_id: &Option<String>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(&(outcome, unexplained, proposal_id)).expect("open-set receipt serializes"));
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenSetInvestigation {
    pub id: String,
    pub epistemic: EpistemicInvestigation,
    pub expected_outcome: OpenSetOutcome,
    pub true_hypothesis: Option<HypothesisId>,
    /// Hidden extension used only to measure recovery after later discovery.
    pub introduced_hypothesis: Option<Hypothesis>,
}

fn reliable_evidence(investigation: &EpistemicInvestigation) -> Vec<&EvidenceRecord> {
    let as_of = investigation.evidence.iter().map(|record| record.timestamp).max().unwrap_or(0);
    investigation.evidence.iter().filter(|record| record.valid_at(as_of) && u16::from(record.reliability) * u16::from(record.confidence) >= MIN_DECISIVE_EVIDENCE_SCORE).collect()
}

fn information_gain_for_query(hypotheses: &[Hypothesis], query_id: &str, novel_outcome: &str) -> InformationGain {
    let mut partitions = BTreeMap::<String, u64>::new();
    for hypothesis in hypotheses {
        let outcome = hypothesis.predictions.get(query_id).cloned().unwrap_or_else(|| "unmodeled".into());
        *partitions.entry(outcome).or_insert(0) += 1;
    }
    *partitions.entry(novel_outcome.into()).or_insert(0) += 1;
    let total = hypotheses.len() as u64 + 1;
    let sum_squares = partitions.values().map(|size| size * size).sum::<u64>();
    InformationGain { numerator: total * total - sum_squares, denominator: total }
}

fn proposal_for(unexplained: &[&EvidenceRecord], investigation: &EpistemicInvestigation) -> Option<HypothesisProposal> {
    let record = unexplained.first()?;
    let mut predictions = BTreeMap::new();
    predictions.insert(record.query_id.clone(), record.outcome.clone());
    let overlap_hypotheses = investigation.hypotheses.iter().filter(|hypothesis| hypothesis.predictions.contains_key(&record.query_id)).map(|hypothesis| hypothesis.id.clone()).collect::<Vec<_>>();
    let expected_information_gain = information_gain_for_query(&investigation.hypotheses, &record.query_id, &record.outcome);
    let proposal_id = format!("novel:{}:{}", record.query_id, record.outcome);
    Some(HypothesisProposal {
        proposal_id,
        unexplained_observations: unexplained.iter().map(|item| item.id.clone()).collect(),
        minimum_latent_cause: format!("an unmodeled cause producing {} for {}", record.outcome, record.query_id),
        predictions,
        overlap_hypotheses,
        assumptions: vec![format!("the {} observation is reliable", record.id)],
        falsification_conditions: vec![format!("reliable evidence on {} matches the active hypotheses instead", record.query_id)],
        expected_information_gain,
    })
}

pub fn analyze_open_set(investigation: &OpenSetInvestigation) -> OpenSetAnalysis {
    let epistemic = analyze(&investigation.epistemic);
    let reliable = reliable_evidence(&investigation.epistemic);
    let unexplained = reliable.iter().filter(|record| investigation.epistemic.hypotheses.iter().all(|hypothesis| hypothesis.predictions.get(&record.query_id) != Some(&record.outcome))).copied().collect::<Vec<_>>();
    let has_evidence = !investigation.epistemic.evidence.is_empty();
    let outcome = if reliable.is_empty() {
        OpenSetOutcome::InsufficientEvidence
    } else if !unexplained.is_empty() {
        if proposal_for(&unexplained, &investigation.epistemic).is_some() { OpenSetOutcome::NovelHypothesisNeeded } else { OpenSetOutcome::NoAdequateHypothesis }
    } else if epistemic.plausible_hypotheses.len() == 1 {
        OpenSetOutcome::BestKnownHypothesis { hypothesis_id: epistemic.plausible_hypotheses[0].clone() }
    } else if epistemic.plausible_hypotheses.len() > 1 {
        OpenSetOutcome::MultiplePlausibleHypotheses { hypothesis_ids: epistemic.plausible_hypotheses.clone() }
    } else if has_evidence {
        OpenSetOutcome::NoAdequateHypothesis
    } else {
        OpenSetOutcome::InsufficientEvidence
    };
    OpenSetAnalysis { epistemic, outcome, unexplained_observations: unexplained.iter().map(|record| record.id.clone()).collect(), proposal: proposal_for(&unexplained, &investigation.epistemic) }
}

pub fn replay_open_set(investigation: &OpenSetInvestigation) -> OpenSetReplayReceipt {
    let analysis = analyze_open_set(investigation);
    let proposal_id = analysis.proposal.as_ref().map(|proposal| proposal.proposal_id.clone());
    let replay_hash = open_set_hash(&analysis.outcome, &analysis.unexplained_observations, &proposal_id);
    OpenSetReplayReceipt { investigation_id: investigation.id.clone(), outcome: analysis.outcome, unexplained_observations: analysis.unexplained_observations, proposal_id, replay_hash }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenSetBenchmarkReport {
    pub cases: usize,
    pub outcomes_correct: usize,
    pub novelty_cases: usize,
    pub missing_hypothesis_detected: usize,
    pub proposal_cases: usize,
    pub proposals_falsifiable: usize,
    pub recovery_cases: usize,
    pub recovery_correct: usize,
    pub calibration_cases: usize,
    pub calibration_retained: usize,
    pub recommendation_quality: usize,
    pub replay_verified: usize,
}

pub fn evaluate_corpus(cases: &[OpenSetInvestigation]) -> OpenSetBenchmarkReport {
    let mut report = OpenSetBenchmarkReport { cases: cases.len(), ..Default::default() };
    for case in cases {
        let analysis = analyze_open_set(case);
        let receipt = replay_open_set(case);
        report.outcomes_correct += usize::from(analysis.outcome == case.expected_outcome);
        if matches!(case.expected_outcome, OpenSetOutcome::NovelHypothesisNeeded) {
            report.novelty_cases += 1;
            report.missing_hypothesis_detected += usize::from(matches!(analysis.outcome, OpenSetOutcome::NovelHypothesisNeeded));
            report.proposal_cases += 1;
            report.proposals_falsifiable += usize::from(analysis.proposal.as_ref().is_some_and(|proposal| !proposal.falsification_conditions.is_empty()));
        }
        let recovered = case.introduced_hypothesis.as_ref().map(|introduced| {
            let mut expanded = case.epistemic.clone();
            expanded.hypotheses.push(introduced.clone());
            matches!(analyze(&expanded), EpistemicAnalysis { plausible_hypotheses, .. } if plausible_hypotheses.contains(&introduced.id))
        }).unwrap_or(true);
        if case.introduced_hypothesis.is_some() {
            report.recovery_cases += 1;
            report.recovery_correct += usize::from(recovered);
        }
        if case.true_hypothesis.as_ref().is_some_and(|truth| case.epistemic.hypotheses.iter().any(|hypothesis| &hypothesis.id == truth)) {
            report.calibration_cases += 1;
            report.calibration_retained += usize::from(analysis.epistemic.plausible_hypotheses.contains(case.true_hypothesis.as_ref().expect("known truth")));
        }
        report.recommendation_quality += usize::from(analysis.proposal.as_ref().is_none_or(|proposal| proposal.expected_information_gain.numerator > 0));
        report.replay_verified += usize::from(receipt.replay_verified());
    }
    report
}

fn hypothesis(id: &str, query: &str, outcome: &str) -> Hypothesis {
    Hypothesis { id: HypothesisId(id.into()), description: format!("hypothesis {id}"), predictions: [(query.into(), outcome.into())].into_iter().collect() }
}

fn query(id: &str) -> crate::epistemic::EvidenceQuery {
    crate::epistemic::EvidenceQuery { id: id.into(), description: format!("test {id}"), cost: 1 }
}

fn evidence(id: &str, query_id: &str, outcome: &str, reliability: u8, valid_until: Option<u64>) -> EvidenceRecord {
    EvidenceRecord { id: id.into(), query_id: query_id.into(), outcome: outcome.into(), timestamp: 1, valid_until, source: format!("sensor-{id}"), reliability, confidence: 90 }
}

pub fn synthetic_corpus() -> Vec<OpenSetInvestigation> {
    let mut cases = Vec::with_capacity(300);
    for index in 0..80 {
        let hypotheses = vec![hypothesis("h-a", "q", "a"), hypothesis("h-b", "q", "b"), hypothesis("h-c", "q", "c")];
        let base = EpistemicInvestigation { id: format!("open-included-{index:03}"), hypotheses, queries: vec![query("q")], evidence: vec![evidence("included", "q", "a", 100, None)], ground_truth: Some(HypothesisId("h-a".into())), expected_recommendation: Recommendation::NoDiscriminatingEvidence };
        cases.push(OpenSetInvestigation { id: base.id.clone(), epistemic: base, expected_outcome: OpenSetOutcome::BestKnownHypothesis { hypothesis_id: HypothesisId("h-a".into()) }, true_hypothesis: Some(HypothesisId("h-a".into())), introduced_hypothesis: None });
    }
    for index in 0..80 {
        let hypotheses = vec![hypothesis("h-a", "q", "a"), hypothesis("h-b", "q", "b")];
        let base = EpistemicInvestigation { id: format!("open-omitted-{index:03}"), hypotheses, queries: vec![query("q")], evidence: vec![evidence("omitted", "q", "c", 100, None)], ground_truth: Some(HypothesisId("h-c".into())), expected_recommendation: Recommendation::NoDiscriminatingEvidence };
        cases.push(OpenSetInvestigation { id: base.id.clone(), epistemic: base, expected_outcome: OpenSetOutcome::NovelHypothesisNeeded, true_hypothesis: Some(HypothesisId("h-c".into())), introduced_hypothesis: Some(hypothesis("h-c", "q", "c")) });
    }
    for index in 0..40 {
        let hypotheses = vec![hypothesis("h-a", "q", "a"), hypothesis("h-b", "q", "b")];
        let base = EpistemicInvestigation { id: format!("open-inadequate-{index:03}"), hypotheses, queries: vec![query("q")], evidence: vec![evidence("anomaly", "q", "unknown", 100, None)], ground_truth: None, expected_recommendation: Recommendation::NoDiscriminatingEvidence };
        cases.push(OpenSetInvestigation { id: base.id.clone(), epistemic: base, expected_outcome: OpenSetOutcome::NovelHypothesisNeeded, true_hypothesis: None, introduced_hypothesis: Some(hypothesis("h-new", "q", "unknown")) });
    }
    for index in 0..30 {
        let hypotheses = vec![hypothesis("h-a", "q", "a"), hypothesis("h-b", "q", "b")];
        let base = EpistemicInvestigation { id: format!("open-misleading-{index:03}"), hypotheses, queries: vec![query("q")], evidence: vec![evidence("misleading", "q", "a", 100, None)], ground_truth: Some(HypothesisId("h-b".into())), expected_recommendation: Recommendation::NoDiscriminatingEvidence };
        cases.push(OpenSetInvestigation { id: base.id.clone(), epistemic: base, expected_outcome: OpenSetOutcome::BestKnownHypothesis { hypothesis_id: HypothesisId("h-a".into()) }, true_hypothesis: Some(HypothesisId("h-b".into())), introduced_hypothesis: None });
    }
    for index in 0..20 {
        let hypotheses = vec![hypothesis("h-a", "q", "a"), hypothesis("h-b", "q", "b")];
        let base = EpistemicInvestigation { id: format!("open-correlated-{index:03}"), hypotheses, queries: vec![query("q")], evidence: vec![evidence("sensor-a", "q", "a", 90, None), evidence("sensor-b", "q", "a", 90, None)], ground_truth: Some(HypothesisId("h-a".into())), expected_recommendation: Recommendation::NoDiscriminatingEvidence };
        cases.push(OpenSetInvestigation { id: base.id.clone(), epistemic: base, expected_outcome: OpenSetOutcome::BestKnownHypothesis { hypothesis_id: HypothesisId("h-a".into()) }, true_hypothesis: Some(HypothesisId("h-a".into())), introduced_hypothesis: None });
    }
    for index in 0..20 {
        let hypotheses = vec![hypothesis("h-a", "q", "a"), hypothesis("h-b", "q", "b")];
        let base = EpistemicInvestigation { id: format!("open-stale-{index:03}"), hypotheses, queries: vec![query("q")], evidence: vec![evidence("stale", "q", "a", 100, Some(0))], ground_truth: Some(HypothesisId("h-a".into())), expected_recommendation: Recommendation::NoDiscriminatingEvidence };
        cases.push(OpenSetInvestigation { id: base.id.clone(), epistemic: base, expected_outcome: OpenSetOutcome::InsufficientEvidence, true_hypothesis: Some(HypothesisId("h-a".into())), introduced_hypothesis: None });
    }
    for index in 0..20 {
        let hypotheses = vec![hypothesis("h-a", "q", "a"), hypothesis("h-b", "q", "b")];
        let base = EpistemicInvestigation { id: format!("open-adversarial-{index:03}"), hypotheses, queries: vec![query("q")], evidence: vec![evidence("adversarial", "q", "unknown", 100, None)], ground_truth: None, expected_recommendation: Recommendation::NoDiscriminatingEvidence };
        cases.push(OpenSetInvestigation { id: base.id.clone(), epistemic: base, expected_outcome: OpenSetOutcome::NovelHypothesisNeeded, true_hypothesis: None, introduced_hypothesis: Some(hypothesis("h-new", "q", "unknown")) });
    }
    for index in 0..10 {
        let hypotheses = vec![hypothesis("h-a", "q", "same"), hypothesis("h-b", "q", "same")];
        let base = EpistemicInvestigation { id: format!("open-no-preference-{index:03}"), hypotheses, queries: vec![query("q")], evidence: vec![evidence("shared", "q", "same", 100, None)], ground_truth: None, expected_recommendation: Recommendation::NoDiscriminatingEvidence };
        cases.push(OpenSetInvestigation { id: base.id.clone(), epistemic: base, expected_outcome: OpenSetOutcome::MultiplePlausibleHypotheses { hypothesis_ids: vec![HypothesisId("h-a".into()), HypothesisId("h-b".into())] }, true_hypothesis: None, introduced_hypothesis: None });
    }
    cases
}

pub fn synthetic_corpus_hash() -> String {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(&synthetic_corpus()).expect("open-set corpus serializes"));
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_set_detects_missing_hypotheses_without_promotion() {
        let cases = synthetic_corpus();
        let report = evaluate_corpus(&cases);
        eprintln!("phase7 open-set: hash={} cases={} outcomes={} novelty={}/{} falsifiable={}/{} recovery={}/{} calibration={}/{} recommendation={} replay={}", synthetic_corpus_hash(), report.cases, report.outcomes_correct, report.missing_hypothesis_detected, report.novelty_cases, report.proposals_falsifiable, report.proposal_cases, report.recovery_correct, report.recovery_cases, report.calibration_retained, report.calibration_cases, report.recommendation_quality, report.replay_verified);
        assert_eq!(report.cases, 300);
        assert_eq!(report.outcomes_correct, 300);
        assert_eq!(report.novelty_cases, 140);
        assert_eq!(report.missing_hypothesis_detected, 140);
        assert_eq!(report.proposal_cases, 140);
        assert_eq!(report.proposals_falsifiable, 140);
        assert_eq!(report.recovery_cases, 140);
        assert_eq!(report.recovery_correct, 140);
        assert_eq!(report.calibration_cases, 150);
        assert_eq!(report.calibration_retained, 120);
        assert_eq!(report.recommendation_quality, 300);
        assert_eq!(report.replay_verified, 300);
        assert!(matches!(analyze_open_set(&cases[80]).outcome, OpenSetOutcome::NovelHypothesisNeeded));
        assert!(analyze_open_set(&cases[80]).proposal.is_some());
        let mut tampered = replay_open_set(&cases[80]);
        tampered.proposal_id = None;
        assert!(!tampered.replay_verified());
    }
}
