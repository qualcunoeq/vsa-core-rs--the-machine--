//! Pressure tests for unit-aware artifacts crossing existing typed stages.

use crate::linear_system::{
    execute_linear_system, replay_linear_system, LinearSystemExecutionReceipt,
};
use crate::quantity_relation_integration::{bridge_to_algebra, AlgebraBridgeReceipt};
use crate::unit_aware_quantity::UnitQuantityArtifact;
use regex::Regex;

#[derive(Debug, Clone)]
pub struct UnitQuantityCompositionReceipt {
    pub unit_replay_verified: bool,
    pub relation_replay_verified: bool,
    pub algebra: AlgebraBridgeReceipt,
}

pub fn compose_to_algebra(
    artifact: &UnitQuantityArtifact,
) -> Option<UnitQuantityCompositionReceipt> {
    if !artifact.replay_verified() {
        return None;
    }
    let relation = artifact.to_quantity_relation();
    if !relation.replay_verified() {
        return None;
    }
    let algebra = bridge_to_algebra(&relation)?;
    Some(UnitQuantityCompositionReceipt {
        unit_replay_verified: true,
        relation_replay_verified: true,
        algebra,
    })
}

/// Convert an explicit integer unit conversion into a two-equation system.
/// This is intentionally limited to conversion artifacts whose exact
/// expression is `amount * factor` or `amount / factor`; no dimensional
/// inference is performed.
pub fn compose_conversion_to_linear_system(
    artifact: &UnitQuantityArtifact,
) -> Option<LinearSystemExecutionReceipt> {
    if !artifact.replay_verified() || artifact.operation != "conversion" {
        return None;
    }
    let multiplication = Regex::new(r"^(\d+) \* (\d+)$").ok()?;
    let division = Regex::new(r"^(\d+) / (\d+)$").ok()?;
    let prompt = if let Some(caps) = multiplication.captures(&artifact.expression) {
        format!(
            "Solve system: x = {}; y - {}*x = 0 for x,y",
            &caps[1], &caps[2]
        )
    } else if let Some(caps) = division.captures(&artifact.expression) {
        format!(
            "Solve system: x = {}; {}*y - x = 0 for x,y",
            &caps[1], &caps[2]
        )
    } else {
        return None;
    };
    let receipt = execute_linear_system(&prompt).ok()?;
    replay_linear_system(&receipt).then_some(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit_aware_quantity::{formalize, UnitQuantityDecision};

    #[test]
    fn unit_quantity_algebra_chain_replays() {
        let UnitQuantityDecision::Accepted(artifact) =
            formalize("Add 2 meters and 30 centimeters; express the total in centimeters.")
        else {
            panic!("unit relation not accepted");
        };
        let receipt = compose_to_algebra(&artifact).expect("composed algebra route");
        assert!(receipt.unit_replay_verified && receipt.relation_replay_verified);
        assert!(receipt.algebra.algebra_replay_verified);
        assert_eq!(receipt.algebra.result, "230");
    }

    #[test]
    fn conversion_can_cross_into_a_replayed_linear_system() {
        let UnitQuantityDecision::Accepted(artifact) =
            formalize("Convert 3 meters to centimeters using 100 centimeters per meter.")
        else {
            panic!("conversion not accepted");
        };
        let receipt = compose_conversion_to_linear_system(&artifact).expect("system route");
        assert!(receipt.replay_verified);
        assert_eq!(receipt.solution.get("y").map(String::as_str), Some("300"));
    }
}
