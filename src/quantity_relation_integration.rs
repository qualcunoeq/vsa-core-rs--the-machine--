//! Explicit, typed handoffs from QuantityRelation artifacts to existing
//! algebra and linear-system executors.  These bridges are diagnostic
//! integration tests; they do not register QuantityRelation globally.

use crate::algebra_island;
use crate::linear_system;
use crate::quantity_relation::QuantityRelationArtifact;
use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlgebraBridgeReceipt {
    pub source_signature: String,
    pub prompt: String,
    pub result: String,
    pub relation_replay_verified: bool,
    pub algebra_replay_verified: bool,
}

pub fn bridge_to_algebra(artifact: &QuantityRelationArtifact) -> Option<AlgebraBridgeReceipt> {
    if !artifact.replay_verified() {
        return None;
    }
    let expression = artifact.algebra_expression.as_ref()?;
    let prompt = format!("Evaluate {expression}");
    let answer = algebra_island::try_answer(&prompt)?;
    if !answer.receipt.verification.passed {
        return None;
    }
    let replay = algebra_island::try_answer(&prompt)?;
    if replay.answer != answer.answer || !replay.receipt.verification.passed {
        return None;
    }
    Some(AlgebraBridgeReceipt {
        source_signature: artifact.signature.clone(),
        prompt,
        result: answer.answer,
        relation_replay_verified: true,
        algebra_replay_verified: true,
    })
}

/// Convert a ratio artifact with an explicit anchor into a two-variable
/// system.  This is intentionally the only QuantityRelation → linear-system
/// bridge in V1; other families remain algebra-only until a separate contract
/// is reviewed.
pub fn bridge_ratio_to_linear_system(
    artifact: &QuantityRelationArtifact,
) -> Option<linear_system::LinearSystemExecutionReceipt> {
    if artifact.family != "ratio" || !artifact.replay_verified() {
        return None;
    }
    let ratio = Regex::new(r"blue/red = (\d+)/(\d+)").ok()?;
    let anchor = Regex::new(r"red = (\d+)").ok()?;
    let ratio = ratio.captures(&artifact.constraints[0].lhs)?;
    let anchor = anchor.captures(&artifact.constraints[0].rhs)?;
    let right = &ratio[1];
    let left = &ratio[2];
    let red = &anchor[1];
    let prompt = format!("Solve system: x = {red}; {left}*y - {right}*x = 0 for x,y");
    let receipt = linear_system::execute_linear_system(&prompt).ok()?;
    linear_system::replay_linear_system(&receipt).then_some(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quantity_relation::{formalize, QuantityRelationDecision};

    #[test]
    fn algebra_handoff_replays() {
        let QuantityRelationDecision::Accepted(artifact) =
            formalize("5 notebooks cost 20 dollars. What is the price per notebook?")
        else {
            panic!("relation was not accepted");
        };
        let receipt = bridge_to_algebra(&artifact).expect("algebra bridge");
        assert_eq!(receipt.result, "4");
        assert!(receipt.relation_replay_verified && receipt.algebra_replay_verified);
    }

    #[test]
    fn anchored_ratio_handoff_replays_as_a_system() {
        let QuantityRelationDecision::Accepted(artifact) = formalize(
            "The ratio of red beads to blue beads is 2:3. If there are 8 red beads, how many blue beads are there?",
        ) else {
            panic!("relation was not accepted");
        };
        let receipt = bridge_ratio_to_linear_system(&artifact).expect("system bridge");
        assert!(receipt.replay_verified);
    }
}
