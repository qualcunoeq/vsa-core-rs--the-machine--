//! Shadow lowering from a justified `TargetContextBundle` to a typed problem
//! specification. Lowering stops before method selection or solving.

use crate::target_context::{ContextStatus, RegionRole, TargetContextBundle};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProblemType {
    ScalarEquation,
    PropertyClassification,
    SymbolicExpression,
    OperatorEvaluation,
    CoupledConstraint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoweringStatus {
    Complete,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquationProblemSpec {
    pub status: LoweringStatus,
    pub problem_type: Option<ProblemType>,
    pub requested_target: String,
    pub requested_operation: String,
    pub target_dependencies: Vec<String>,
    pub symbol_table: Vec<String>,
    pub declarations: Vec<String>,
    pub equations: Vec<String>,
    pub relational_constraints: Vec<String>,
    pub assumptions: Vec<String>,
    pub indexed_or_operator_declarations: Vec<String>,
    pub unresolved_constraints: Vec<String>,
    pub provenance_region_ids: Vec<String>,
    pub lowering_reason: String,
    pub lowering_hash: String,
    pub downstream_authorized: bool,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("lowering serializes"))
    )
}

fn replay_payload(spec: &EquationProblemSpec) -> impl Serialize + '_ {
    (
        spec.status,
        spec.problem_type,
        &spec.requested_target,
        &spec.requested_operation,
        &spec.target_dependencies,
        &spec.symbol_table,
        &spec.declarations,
        &spec.equations,
        &spec.relational_constraints,
        &spec.assumptions,
        &spec.indexed_or_operator_declarations,
        &spec.unresolved_constraints,
        &spec.provenance_region_ids,
        &spec.lowering_reason,
        spec.downstream_authorized,
    )
}

impl EquationProblemSpec {
    pub fn replay_verified(&self) -> bool {
        self.lowering_hash == digest(&replay_payload(self))
            && !self.requested_target.is_empty()
            && !self.downstream_authorized
            && (!self.provenance_region_ids.is_empty()
                || self.status == LoweringStatus::Unsupported)
    }
}

fn problem_type(bundle: &TargetContextBundle) -> Option<ProblemType> {
    let operation = bundle.requested_operation.to_ascii_lowercase();
    let target = bundle.target.to_ascii_lowercase();
    if operation.contains("classif")
        || operation.contains("minimum")
        || operation.contains("maximum")
        || operation.contains("predicate")
    {
        Some(ProblemType::PropertyClassification)
    } else if target.contains('α')
        || target.contains('β')
        || target.contains('χ')
        || target.contains('+')
    {
        Some(ProblemType::SymbolicExpression)
    } else if operation.contains("operator") || target.contains('(') {
        Some(ProblemType::OperatorEvaluation)
    } else if bundle.constraints.len() > 1 {
        Some(ProblemType::CoupledConstraint)
    } else if !bundle.constraints.is_empty() {
        Some(ProblemType::ScalarEquation)
    } else {
        None
    }
}

/// Lower a complete context bundle without adding specialist semantics.
pub fn lower_context_bundle(bundle: &TargetContextBundle) -> EquationProblemSpec {
    let mut declarations = Vec::new();
    let mut equations = Vec::new();
    let mut relational_constraints = Vec::new();
    let mut indexed_or_operator_declarations = Vec::new();
    let mut symbol_table: BTreeSet<String> = bundle.symbols.iter().cloned().collect();
    for region in &bundle.included_regions {
        symbol_table.extend(region.symbols.iter().cloned());
        match region.role {
            RegionRole::Declaration | RegionRole::Definition => {
                declarations.push(region.text.clone())
            }
            RegionRole::Constraint => {
                if region.text.contains('=') {
                    equations.push(region.text.clone());
                } else {
                    relational_constraints.push(region.text.clone());
                }
            }
            RegionRole::Assumption => {}
            RegionRole::Incidental | RegionRole::Quoted => {}
        }
        if region.text.contains('(') || region.text.contains('_') {
            indexed_or_operator_declarations.push(region.text.clone());
        }
    }
    let mut unresolved_constraints = bundle.unresolved_alternatives.clone();
    let selected_type = problem_type(bundle);
    let status = if bundle.status == ContextStatus::Ambiguous {
        LoweringStatus::Ambiguous
    } else if bundle.status == ContextStatus::Unsupported || selected_type.is_none() {
        if selected_type.is_none() {
            unresolved_constraints.push("target has no supported typed problem family".into());
        }
        LoweringStatus::Unsupported
    } else {
        LoweringStatus::Complete
    };
    let reason = match status {
        LoweringStatus::Complete => {
            "context lowered to a typed problem specification; method selection deferred"
        }
        LoweringStatus::Ambiguous => "context ambiguity preserved during lowering",
        LoweringStatus::Unsupported => "target/context has no bounded problem representation",
    };
    let mut spec = EquationProblemSpec {
        status,
        problem_type: selected_type,
        requested_target: bundle.target.clone(),
        requested_operation: bundle.requested_operation.clone(),
        target_dependencies: bundle.symbols.clone(),
        symbol_table: symbol_table.into_iter().collect(),
        declarations,
        equations,
        relational_constraints,
        assumptions: bundle.assumptions.clone(),
        indexed_or_operator_declarations,
        unresolved_constraints,
        provenance_region_ids: bundle
            .included_regions
            .iter()
            .map(|region| region.id.clone())
            .collect(),
        lowering_reason: reason.into(),
        lowering_hash: String::new(),
        downstream_authorized: false,
    };
    let lowering_hash = digest(&replay_payload(&spec));
    spec.lowering_hash = lowering_hash;
    spec
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::target_context::{
        assemble_target_context, ContextRegion, RegionRole, TargetContextRequest,
    };

    #[test]
    fn lowers_property_without_forcing_scalar_equations() {
        let region = ContextRegion {
            id: "definition".into(),
            role: RegionRole::Definition,
            text: "invariant group".into(),
            symbols: vec!["invariant_group".into()],
            target_links: vec!["topological invariant".into()],
            scope: "root".into(),
            source_spans: vec!["definition".into()],
        };
        let bundle = assemble_target_context(&TargetContextRequest {
            target: "topological invariant".into(),
            target_components: vec!["invariant_group".into()],
            requested_operation: "classify invariant group".into(),
            regions: vec![region],
        });
        let spec = lower_context_bundle(&bundle);
        assert_eq!(spec.status, LoweringStatus::Complete);
        assert_eq!(spec.problem_type, Some(ProblemType::PropertyClassification));
        assert!(spec.replay_verified());
    }

    #[test]
    fn preserves_ambiguous_context() {
        let bundle = TargetContextBundle {
            status: ContextStatus::Ambiguous,
            target: "x".into(),
            requested_operation: "compute".into(),
            included_regions: Vec::new(),
            excluded_region_ids: Vec::new(),
            symbols: vec!["x".into()],
            assumptions: Vec::new(),
            constraints: Vec::new(),
            dependencies: Default::default(),
            unresolved_alternatives: vec!["two scopes".into()],
            binding_handoff_ready: false,
            replay_hash: String::new(),
            downstream_authorized: false,
        };
        let spec = lower_context_bundle(&bundle);
        assert_eq!(spec.status, LoweringStatus::Ambiguous);
        assert!(!spec.replay_verified());
    }
}
