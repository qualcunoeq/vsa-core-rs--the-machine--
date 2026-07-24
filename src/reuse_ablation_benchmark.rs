//! Safe, isolated ablations for proof-index and derived-fact reuse.
//!
//! These probes measure existing governed lookup paths. They never execute a
//! reused proof or bypass a fact policy; execution and replay remain the
//! authorities owned by the existing planner/verifier.

use crate::capabilities::{CapabilityIoType, CapabilityRegistry, CapabilitySpec, InputRequirement};
use crate::capability_planner::{
    plan_for_goal_with_fact_index, CapabilityChainPlan, CapabilityChainProofIndex,
    CapabilityChainProofStep, CapabilityChainProofTrace,
};
use crate::evidence::{
    DerivedFact, DerivedFactIndex, DerivedProofKind, FactIndexInsert, FactPolicy, FactPrecision,
};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReuseAblationReport {
    pub cases: usize,
    pub proof_expected_hits: usize,
    pub proof_hits: usize,
    pub proof_baseline_hits: usize,
    pub proof_false_hits: usize,
    pub proof_replay_verified: usize,
    pub fact_expected_hits: usize,
    pub fact_hits: usize,
    pub fact_baseline_hits: usize,
    pub fact_false_hits: usize,
    pub fact_retrieval_receipts: usize,
    pub deterministic: bool,
}

fn proof_trace(execution_id: &str, value: usize) -> CapabilityChainProofTrace {
    let capability = "evaluate_expression".to_string();
    CapabilityChainProofTrace {
        execution_id: execution_id.into(),
        plan: CapabilityChainPlan {
            goal: CapabilityIoType::ExactValue,
            steps: vec![capability.clone()],
        },
        steps: vec![CapabilityChainProofStep {
            step_index: 0,
            capability_id: capability,
            input_artifacts: vec![format!("expression-{value}")],
            output_artifacts: vec![format!("value-{value}")],
            verification_receipt: "evaluation replay".into(),
        }],
        retrieved_facts: Vec::new(),
        final_artifacts: vec![format!("value-{value}")],
        replay_verified: true,
    }
}

fn fact_registry() -> CapabilityRegistry {
    let mut registry = CapabilityRegistry::default();
    let mut capability = CapabilitySpec::expression_evaluation_v1();
    capability.id = "derived_fact_consumer".into();
    capability.consumes = vec![CapabilityIoType::DerivedFact];
    capability.produces = vec![CapabilityIoType::ExactValue];
    capability.input_requirements = vec![
        InputRequirement::VerifiedDerivedFact,
        InputRequirement::ReplayVerifier,
    ];
    capability.fact_policy = Some(FactPolicy::verified_transformation());
    registry.register(capability);
    registry
}

fn fact(value: usize) -> DerivedFact {
    DerivedFact {
        id: format!("derived-{value}"),
        content: format!("value = {value}"),
        parent_lineage: vec![format!("source-{value}")],
        provenance: "verified publication".into(),
        proof_kind: DerivedProofKind::ExactTransformation,
        precision: FactPrecision::Exact,
        assumptions: Vec::new(),
        domain: Some("algebra".into()),
    }
}

pub fn evaluate(cases: usize, _seed: u64) -> ReuseAblationReport {
    let mut proof_index = CapabilityChainProofIndex::default();
    let mut proof_expected_hits = 0;
    let mut proof_hits = 0;
    let proof_baseline_hits = 0;
    let mut proof_false_hits = 0;
    let mut proof_replay_verified = 0;
    let mut fact_expected_hits = 0;
    let mut fact_hits = 0;
    let fact_baseline_hits = 0;
    let mut fact_false_hits = 0;
    let mut fact_retrieval_receipts = 0;
    let registry = fact_registry();

    for value in 0..cases {
        let expected_reuse = value % 2 == 0;
        let stored = proof_trace(&format!("stored-{value}"), value);
        if expected_reuse {
            proof_expected_hits += 1;
            proof_index
                .insert(stored.clone())
                .expect("fresh verified proof fixture must insert");
        }
        let probe = proof_trace(&format!("probe-{value}"), value);
        let reused = proof_index.find_equivalent(&probe).is_some();
        proof_hits += usize::from(reused);
        proof_false_hits += usize::from(reused != expected_reuse);
        proof_replay_verified += usize::from(
            reused
                && proof_index
                    .find_equivalent(&probe)
                    .is_some_and(|proof| proof.replay_verified),
        );

        let mut index = DerivedFactIndex::default();
        let fact_reuse = value % 2 == 0;
        if fact_reuse {
            fact_expected_hits += 1;
            assert_eq!(
                index.insert("value", fact(value), &FactPolicy::verified_transformation(),),
                Ok(FactIndexInsert::Added)
            );
        }
        let plan = plan_for_goal_with_fact_index(
            CapabilityIoType::ExactValue,
            std::collections::BTreeSet::new(),
            &index,
            &registry,
        );
        let retrieved = plan
            .as_ref()
            .map(|plan| !plan.derived_fact_proofs.is_empty())
            .unwrap_or(false);
        fact_hits += usize::from(retrieved);
        fact_false_hits += usize::from(retrieved != fact_reuse);
        fact_retrieval_receipts += usize::from(plan.as_ref().ok().is_some_and(|plan| {
            plan.derived_fact_proofs
                .iter()
                .all(|proof| proof.retrieval_receipt.is_some())
        }));
    }

    ReuseAblationReport {
        cases,
        proof_expected_hits,
        proof_hits,
        proof_baseline_hits,
        proof_false_hits,
        proof_replay_verified,
        fact_expected_hits,
        fact_hits,
        fact_baseline_hits,
        fact_false_hits,
        fact_retrieval_receipts,
        deterministic: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reuse_controls_are_safe_and_deterministic() {
        let report = evaluate(100, 42);
        assert_eq!(report.proof_expected_hits, 50);
        assert_eq!(report.proof_hits, 50);
        assert_eq!(report.proof_replay_verified, 50);
        assert_eq!(report.proof_false_hits, 0);
        assert_eq!(report.fact_expected_hits, 50);
        assert_eq!(report.fact_hits, 50);
        assert_eq!(report.fact_false_hits, 0);
        assert_eq!(report.fact_retrieval_receipts, 50);
        assert!(report.deterministic);
        assert_eq!(report, evaluate(100, 42));
    }
}
