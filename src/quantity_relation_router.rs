//! Diagnostic mixed router for the new QuantityRelation vertical.
//!
//! QuantityRelation is tried only when it yields a unique typed artifact.  If
//! it abstains, the pre-existing raw decomposition path gets the prompt.  No
//! route returned here authorizes execution by itself.

use crate::quantity_relation::{formalize, QuantityRelationArtifact, QuantityRelationDecision};
use crate::raw_decomposition_benchmark::{decompose, DecompositionDecision};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MixedRouteDecision {
    QuantityRelation(QuantityRelationArtifact),
    Existing(DecompositionDecision),
    Ambiguous,
    Unsupported,
}

pub fn route(prompt: &str) -> MixedRouteDecision {
    match formalize(prompt) {
        QuantityRelationDecision::Accepted(artifact) => {
            MixedRouteDecision::QuantityRelation(artifact)
        }
        QuantityRelationDecision::Ambiguous => MixedRouteDecision::Ambiguous,
        QuantityRelationDecision::Unsupported => match decompose(prompt) {
            DecompositionDecision::Sketch(sketch) => {
                MixedRouteDecision::Existing(DecompositionDecision::Sketch(sketch))
            }
            DecompositionDecision::Ambiguous => MixedRouteDecision::Ambiguous,
            DecompositionDecision::NoDecomposition => MixedRouteDecision::Unsupported,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quantity_route_does_not_steal_existing_numeric_route() {
        assert!(matches!(
            route("Compute 2 + 3"),
            MixedRouteDecision::Existing(_)
        ));
        assert!(matches!(
            route("5 notebooks cost 20 dollars. What is the price per notebook?"),
            MixedRouteDecision::QuantityRelation(_)
        ));
        assert!(matches!(
            route("A price changes by 20% each year. What is the final price?"),
            MixedRouteDecision::Unsupported
        ));
    }
}
