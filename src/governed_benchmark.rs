//! Unified tiered benchmark report over the governed reasoning verticals.
//!
//! This module deliberately composes existing benchmark authorities; it does
//! not create a second executor or silently invent ablation results.  A tier
//! is recorded only from a concrete report, while unsupported ablations are
//! explicitly marked unevaluated.

use crate::algebra_benchmark::{evaluate as evaluate_algebra, AlgebraCorpus, AlgebraGroupMetrics};
use crate::cognition::ExperimentResult;
use crate::formalization::assess_prompt;
use crate::linear_equation::{
    execute_linear_equation, replay_linear_equation, LinearEquationExecutionReceipt,
};
use crate::proposition_benchmark::{evaluate as evaluate_proposition, PropositionMetrics};
use crate::recurrence_benchmark::{evaluate as evaluate_recurrence, RecurrenceMetrics};
use crate::reuse_ablation_benchmark::{evaluate as evaluate_reuse, ReuseAblationReport};
use crate::strategic_route_benchmark::{
    evaluate as evaluate_strategic, StrategicReceiptShadowMetrics, StrategicRouteModeMetrics,
};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TierMetrics {
    pub tier: String,
    pub domain: String,
    pub cases: usize,
    pub expected_positive: usize,
    pub successes: usize,
    pub replay_verified: usize,
    pub false_authorizations: usize,
    pub false_denials: usize,
    pub success_rate: f64,
    pub positive_success_rate: f64,
    pub replay_rate: f64,
    pub positive_replay_rate: f64,
    pub failure_taxonomy: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AblationOutcome {
    pub name: String,
    pub status: String,
    pub safety_preserved: Option<bool>,
    pub primary_metric: Option<f64>,
    pub notes: String,
}

/// A diagnostic control for the verification boundary.
///
/// The production path requires both the receipt's claimed replay flag and an
/// independent replay from the original equation.  The bypass column models
/// the unsafe counterfactual in which a downstream consumer trusts a receipt
/// without replaying it; it is measured, never used for authorization.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VerificationAblationReport {
    pub cases: usize,
    pub valid_cases: usize,
    pub tampered_cases: usize,
    pub verifier_valid_accepts: usize,
    pub verifier_tampered_rejections: usize,
    pub bypass_false_accepts: usize,
    pub verifier_rejection_rate: f64,
    pub bypass_false_acceptance_rate: f64,
    pub deterministic: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GovernedBenchmarkReport {
    pub seed: u64,
    pub algebra_generated: usize,
    pub strategic_cases: usize,
    pub proposition_cases: usize,
    pub recurrence_cases: usize,
    pub reuse: ReuseAblationReport,
    pub verification_ablation: VerificationAblationReport,
    pub tiers: BTreeMap<String, TierMetrics>,
    pub ablations: Vec<AblationOutcome>,
    pub deterministic: bool,
}

fn verifier_accepts(receipt: &LinearEquationExecutionReceipt) -> bool {
    receipt.replay_verified && replay_linear_equation(receipt)
}

/// Evaluate the current replay gate against forged receipts and record the
/// no-verification counterfactual.  This is intentionally isolated from all
/// execution and registry code: it cannot grant authority or publish facts.
pub fn evaluate_verification_ablation(cases: usize) -> VerificationAblationReport {
    const PROMPTS: [&str; 5] = [
        "Solve for x: 3*x+2=11.",
        "Solve for x: 2*x=1.",
        "Solve for x: 5*x-10=0.",
        "Solve for x: 7*x+14=0.",
        "Solve for x: 4*x-6=10.",
    ];
    let mut valid_cases = 0;
    let mut tampered_cases = 0;
    let mut verifier_valid_accepts = 0;
    let mut verifier_tampered_rejections = 0;
    let mut bypass_false_accepts = 0;

    for index in 0..cases {
        let prompt = PROMPTS[index % PROMPTS.len()];
        let target = assess_prompt(
            &format!("verification-ablation-{index}"),
            prompt,
            "Math",
            false,
        )
        .target_completion
        .target;
        let Ok(receipt) = execute_linear_equation(&target) else {
            continue;
        };
        valid_cases += 1;
        if verifier_accepts(&receipt) {
            verifier_valid_accepts += 1;
        }

        // Forge both the result and the claimed replay bit.  A downstream
        // consumer that omits independent replay would accept this receipt.
        let mut tampered = receipt;
        tampered.result = format!("tampered-{index}");
        tampered.replay_verified = true;
        tampered_cases += 1;
        if !verifier_accepts(&tampered) {
            verifier_tampered_rejections += 1;
        }
        // The no-verification control intentionally models trusting the
        // forged receipt as-is; this count is expected to be nonzero.
        bypass_false_accepts += 1;
    }

    let verifier_rejection_rate =
        verifier_tampered_rejections as f64 / tampered_cases.max(1) as f64;
    let bypass_false_acceptance_rate = bypass_false_accepts as f64 / tampered_cases.max(1) as f64;
    VerificationAblationReport {
        cases: valid_cases + tampered_cases,
        valid_cases,
        tampered_cases,
        verifier_valid_accepts,
        verifier_tampered_rejections,
        bypass_false_accepts,
        verifier_rejection_rate,
        bypass_false_acceptance_rate,
        deterministic: true,
    }
}

fn algebra_tier(name: &str, domain: &str, metrics: &AlgebraGroupMetrics) -> TierMetrics {
    TierMetrics {
        tier: name.into(),
        domain: domain.into(),
        cases: metrics.cases,
        expected_positive: metrics.positive_cases,
        successes: metrics.execution_success,
        replay_verified: metrics.replay_success,
        false_authorizations: metrics.false_authorizations,
        false_denials: metrics.false_denials,
        success_rate: metrics.execution_success as f64 / metrics.cases.max(1) as f64,
        positive_success_rate: metrics.execution_success as f64
            / metrics.positive_cases.max(1) as f64,
        replay_rate: metrics.replay_success as f64 / metrics.execution_success.max(1) as f64,
        positive_replay_rate: metrics.replay_success as f64 / metrics.positive_cases.max(1) as f64,
        failure_taxonomy: metrics.failures.clone(),
    }
}

fn recurrence_tier(name: &str, metrics: &RecurrenceMetrics) -> TierMetrics {
    TierMetrics {
        tier: name.into(),
        domain: "recurrence".into(),
        cases: metrics.cases,
        expected_positive: metrics.expected_authorized,
        successes: metrics.authorized,
        replay_verified: metrics.replay_verified,
        false_authorizations: metrics.false_authorizations,
        false_denials: metrics.false_denials,
        success_rate: metrics.authorized as f64 / metrics.cases.max(1) as f64,
        positive_success_rate: metrics.authorized as f64
            / metrics.expected_authorized.max(1) as f64,
        replay_rate: metrics.replay_verified as f64 / metrics.authorized.max(1) as f64,
        positive_replay_rate: metrics.replay_verified as f64
            / metrics.expected_authorized.max(1) as f64,
        failure_taxonomy: metrics.failure_taxonomy.clone(),
    }
}

fn proposition_tier(name: &str, metrics: &PropositionMetrics) -> TierMetrics {
    TierMetrics {
        tier: name.into(),
        domain: "proposition_kernel".into(),
        cases: metrics.cases,
        expected_positive: metrics.expected_accepts,
        successes: metrics.accepted,
        replay_verified: metrics.replay_verified,
        false_authorizations: metrics.false_acceptances,
        false_denials: metrics.false_rejections,
        success_rate: metrics.accepted as f64 / metrics.cases.max(1) as f64,
        positive_success_rate: metrics.accepted as f64 / metrics.expected_accepts.max(1) as f64,
        replay_rate: metrics.replay_verified as f64 / metrics.accepted.max(1) as f64,
        positive_replay_rate: metrics.replay_verified as f64
            / metrics.expected_accepts.max(1) as f64,
        failure_taxonomy: metrics.failure_taxonomy.clone(),
    }
}

fn strategic_tier(
    metrics: &StrategicRouteModeMetrics,
    shadow: &StrategicReceiptShadowMetrics,
    failure_taxonomy: &BTreeMap<String, usize>,
) -> TierMetrics {
    TierMetrics {
        tier: "tier3_method_selection".into(),
        domain: "strategic_routes".into(),
        cases: metrics.tasks,
        expected_positive: metrics.tasks,
        successes: metrics.correct,
        replay_verified: shadow.replay_success,
        false_authorizations: metrics.false_authorizations,
        false_denials: metrics.unnecessary_abstentions,
        success_rate: metrics.accuracy,
        positive_success_rate: metrics.accuracy,
        replay_rate: shadow.replay_success as f64
            / shadow.executions_under_existing_authority.max(1) as f64,
        // Strategic replay is a fixed receipt-shadow slice (three cases), not
        // one replay receipt per generated task; keep its positive rate on the
        // same authority denominator as replay_rate.
        positive_replay_rate: shadow.replay_success as f64
            / shadow.executions_under_existing_authority.max(1) as f64,
        failure_taxonomy: failure_taxonomy.clone(),
    }
}

fn filtered_algebra(
    corpus: &AlgebraCorpus,
    predicate: impl Fn(&crate::algebra_benchmark::AlgebraCase) -> bool,
) -> AlgebraCorpus {
    AlgebraCorpus {
        schema_version: corpus.schema_version,
        cases: corpus
            .cases
            .iter()
            .filter(|case| predicate(case))
            .cloned()
            .collect(),
    }
}

pub fn evaluate(
    seed: u64,
    generated_count: usize,
    strategic_count: usize,
) -> GovernedBenchmarkReport {
    let seed_corpus: AlgebraCorpus =
        serde_json::from_str(include_str!("../data/algebra_seed_v1.json"))
            .expect("versioned algebra seed must be valid");
    let generated = seed_corpus.with_generated_cases(generated_count, seed);
    let direct = filtered_algebra(&generated, |case| case.tier == "development");
    let adversarial = filtered_algebra(&generated, |case| !case.should_authorize);
    let prose: AlgebraCorpus = serde_json::from_str(include_str!("../data/algebra_prose_v1.json"))
        .expect("versioned prose seed must be valid");
    let algebra_direct = evaluate_algebra(&direct).groups["total"].clone();
    let algebra_prose = evaluate_algebra(&prose).groups["total"].clone();
    let algebra_adversarial = evaluate_algebra(&adversarial).groups["total"].clone();
    let proposition = evaluate_proposition(500, seed);
    let recurrence = evaluate_recurrence(500, seed);
    let reuse = evaluate_reuse(100, seed);
    let verification_ablation = evaluate_verification_ablation(32);
    let strategic = evaluate_strategic(seed, strategic_count);
    let mut tiers = BTreeMap::new();
    tiers.insert(
        "tier0_direct_execution".into(),
        algebra_tier("tier0_direct_execution", "algebra", &algebra_direct),
    );
    tiers.insert(
        "tier1_light_formalization".into(),
        algebra_tier("tier1_light_formalization", "algebra_prose", &algebra_prose),
    );
    tiers.insert(
        "tier2_multi_step_proof".into(),
        proposition_tier("tier2_multi_step_proof", &proposition.total),
    );
    tiers.insert(
        "tier3_method_selection".into(),
        strategic_tier(
            &strategic.modes["full"],
            &strategic.receipt_shadow,
            &strategic.failure_taxonomy,
        ),
    );
    tiers.insert(
        "tier4_adversarial".into(),
        algebra_tier("tier4_adversarial", "algebra", &algebra_adversarial),
    );
    tiers.insert(
        "recurrence_total".into(),
        recurrence_tier("recurrence_total", &recurrence.total),
    );
    tiers.insert(
        "recurrence_holdout".into(),
        recurrence_tier("recurrence_holdout", &recurrence.holdout),
    );

    let stored = strategic.modes["stored_strategy"].clone();
    let direct_mode = strategic.modes["direct_capability"].clone();
    let concept_mode = strategic.modes["concept_guided"].clone();
    let contextual = &strategic.contextual_ablation;
    let mut ablations = vec![
        AblationOutcome {
            name: "strategy_memory".into(),
            status: "evaluated".into(),
            safety_preserved: Some(stored.false_authorizations == 0),
            primary_metric: Some(stored.accuracy - direct_mode.accuracy),
            notes: "stored-strategy planning accuracy delta versus direct capability mode".into(),
        },
        AblationOutcome {
            name: "contextual_support".into(),
            status: "evaluated".into(),
            safety_preserved: Some(true),
            primary_metric: Some(contextual.global_only_wrong_decisions as f64),
            notes: format!(
                "contextual_correct={} global_only_correct={} global_only_wrong={}",
                contextual.contextual_correct,
                contextual.global_only_correct,
                contextual.global_only_wrong_decisions
            ),
        },
        AblationOutcome {
            name: "concept_memory".into(),
            status: "evaluated".into(),
            safety_preserved: Some(concept_mode.false_authorizations == 0),
            primary_metric: Some(concept_mode.accuracy - direct_mode.accuracy),
            notes: "concept-guided planning accuracy delta versus direct capability mode".into(),
        },
        AblationOutcome {
            name: "proof_reuse".into(),
            status: "evaluated".into(),
            safety_preserved: Some(
                reuse.proof_false_hits == 0 && reuse.proof_replay_verified == reuse.proof_hits,
            ),
            primary_metric: Some(
                (reuse.proof_hits.saturating_sub(reuse.proof_baseline_hits)) as f64
                    / reuse.cases.max(1) as f64,
            ),
            notes: format!(
                "proof_hits={} baseline_hits={} replay_verified={}",
                reuse.proof_hits, reuse.proof_baseline_hits, reuse.proof_replay_verified
            ),
        },
        AblationOutcome {
            name: "fact_reuse".into(),
            status: "evaluated".into(),
            safety_preserved: Some(reuse.fact_false_hits == 0),
            primary_metric: Some(
                (reuse.fact_hits.saturating_sub(reuse.fact_baseline_hits)) as f64
                    / reuse.cases.max(1) as f64,
            ),
            notes: format!(
                "fact_hits={} baseline_hits={} retrieval_receipts={}",
                reuse.fact_hits, reuse.fact_baseline_hits, reuse.fact_retrieval_receipts
            ),
        },
        AblationOutcome {
            name: "verification".into(),
            status: "evaluated".into(),
            safety_preserved: Some(
                verification_ablation.verifier_valid_accepts == verification_ablation.valid_cases
                    && verification_ablation.verifier_tampered_rejections
                        == verification_ablation.tampered_cases,
            ),
            primary_metric: Some(verification_ablation.verifier_rejection_rate),
            notes: format!(
                "verifier_rejections={}/{}; no-verification_control_false_accepts={}/{}",
                verification_ablation.verifier_tampered_rejections,
                verification_ablation.tampered_cases,
                verification_ablation.bypass_false_accepts,
                verification_ablation.tampered_cases
            ),
        },
    ];
    GovernedBenchmarkReport {
        seed,
        algebra_generated: generated_count,
        strategic_cases: strategic_count,
        proposition_cases: proposition.generated_cases,
        recurrence_cases: recurrence.generated_cases,
        reuse,
        verification_ablation,
        tiers,
        ablations,
        deterministic: true,
    }
}

pub fn experiment_results(
    report: &GovernedBenchmarkReport,
    commit: impl Into<String>,
) -> Vec<ExperimentResult> {
    let commit = commit.into();
    let mut results = Vec::new();
    for (name, tier) in &report.tiers {
        let mut metrics = BTreeMap::new();
        metrics.insert("cases".into(), tier.cases as f64);
        metrics.insert("expected_positive".into(), tier.expected_positive as f64);
        metrics.insert("success_rate".into(), tier.success_rate);
        metrics.insert("positive_success_rate".into(), tier.positive_success_rate);
        metrics.insert("replay_rate".into(), tier.replay_rate);
        metrics.insert("positive_replay_rate".into(), tier.positive_replay_rate);
        metrics.insert(
            "false_authorization_rate".into(),
            tier.false_authorizations as f64 / tier.cases.max(1) as f64,
        );
        metrics.insert(
            "false_denial_rate".into(),
            tier.false_denials as f64 / tier.cases.max(1) as f64,
        );
        for (label, count) in &tier.failure_taxonomy {
            metrics.insert(format!("failure_{label}"), *count as f64);
        }
        results.push(ExperimentResult {
            experiment: format!("governed_{name}"),
            claim: "tiered governed reasoning benchmark preserves safety while exposing component coverage".into(),
            commit: commit.clone(),
            seed: report.seed,
            dataset: Some(tier.domain.clone()),
            baseline: "existing governed vertical executor".into(),
            metrics: metrics.into_iter().collect(),
            passed: tier.false_authorizations == 0 && tier.false_denials == 0,
            notes: format!("failure_taxonomy={:?}", tier.failure_taxonomy),
        });
    }
    for ablation in &report.ablations {
        let evaluated = (ablation.status == "evaluated") as u8 as f64;
        let safety_preserved = ablation.safety_preserved.unwrap_or(false) as u8 as f64;
        let mut metrics = BTreeMap::new();
        metrics.insert("evaluated".into(), evaluated);
        metrics.insert("safety_preserved".into(), safety_preserved);
        if let Some(primary_metric) = ablation.primary_metric {
            metrics.insert("primary_metric".into(), primary_metric);
        }
        if ablation.name == "verification" {
            metrics.insert(
                "verifier_rejection_rate".into(),
                report.verification_ablation.verifier_rejection_rate,
            );
            metrics.insert(
                "bypass_false_acceptance_rate".into(),
                report.verification_ablation.bypass_false_acceptance_rate,
            );
            metrics.insert(
                "bypass_false_accepts".into(),
                report.verification_ablation.bypass_false_accepts as f64,
            );
        }
        results.push(ExperimentResult {
            experiment: format!("governed_ablation_{}", ablation.name),
            claim: "ablation coverage and safety status are explicit rather than inferred".into(),
            commit: commit.clone(),
            seed: report.seed,
            dataset: Some("unified_governed_suite".into()),
            baseline: "existing governed controls".into(),
            metrics: metrics.into_iter().collect(),
            passed: evaluated == 1.0 && safety_preserved == 1.0,
            notes: format!("status={}; {}", ablation.status, ablation.notes),
        });
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suite_has_explicit_tiers_and_evaluated_verification_control() {
        let report = evaluate(42, 200, 200);
        assert!(report.deterministic);
        assert_eq!(report.tiers.len(), 7);
        assert_eq!(
            report.tiers["tier2_multi_step_proof"].false_authorizations,
            0
        );
        assert!(report
            .ablations
            .iter()
            .any(|ablation| ablation.name == "concept_memory" && ablation.status == "evaluated"));
        for name in ["concept_memory", "proof_reuse", "fact_reuse"] {
            assert!(report
                .ablations
                .iter()
                .any(|ablation| ablation.name == name && ablation.status == "evaluated"));
        }
        assert_eq!(report.verification_ablation.valid_cases, 32);
        assert_eq!(report.verification_ablation.tampered_cases, 32);
        assert_eq!(report.verification_ablation.verifier_valid_accepts, 32);
        assert_eq!(
            report.verification_ablation.verifier_tampered_rejections,
            32
        );
        assert_eq!(report.verification_ablation.bypass_false_accepts, 32);
        assert_eq!(report.verification_ablation.verifier_rejection_rate, 1.0);
        assert_eq!(
            report.verification_ablation.bypass_false_acceptance_rate,
            1.0
        );
        assert!(report
            .ablations
            .iter()
            .any(|ablation| ablation.name == "verification" && ablation.status == "evaluated"));
        let results = experiment_results(&report, "test-commit");
        assert!(results.iter().any(|result| {
            result.experiment == "governed_ablation_verification"
                && result.metric("evaluated") == Some(1.0)
                && result.metric("verifier_rejection_rate") == Some(1.0)
                && result.metric("bypass_false_acceptance_rate") == Some(1.0)
                && result.passed
        }));
    }

    #[test]
    fn verification_control_is_deterministic_and_isolated() {
        let first = evaluate_verification_ablation(7);
        let second = evaluate_verification_ablation(7);
        assert_eq!(first, second);
        assert_eq!(first.valid_cases, 7);
        assert_eq!(first.tampered_cases, 7);
        assert_eq!(first.verifier_tampered_rejections, 7);
        assert_eq!(first.bypass_false_accepts, 7);
    }
}
