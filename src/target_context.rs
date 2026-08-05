//! Shadow cross-region context assembly for Phase 48.
//!
//! The assembler selects only regions justified by target dependencies. It is
//! deliberately separate from solving and does not authorize a downstream
//! capability.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegionRole {
    Definition,
    Constraint,
    Assumption,
    Declaration,
    Incidental,
    Quoted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextStatus {
    Complete,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextRegion {
    pub id: String,
    pub role: RegionRole,
    pub text: String,
    pub symbols: Vec<String>,
    pub target_links: Vec<String>,
    pub scope: String,
    pub source_spans: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetContextRequest {
    pub target: String,
    pub target_components: Vec<String>,
    pub requested_operation: String,
    pub regions: Vec<ContextRegion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetContextBundle {
    pub status: ContextStatus,
    pub target: String,
    pub requested_operation: String,
    pub included_regions: Vec<ContextRegion>,
    pub excluded_region_ids: Vec<String>,
    pub symbols: Vec<String>,
    pub assumptions: Vec<String>,
    pub constraints: Vec<String>,
    pub dependencies: BTreeMap<String, Vec<String>>,
    pub unresolved_alternatives: Vec<String>,
    pub binding_handoff_ready: bool,
    pub replay_hash: String,
    pub downstream_authorized: bool,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("context serializes"))
    )
}

fn replay_payload(bundle: &TargetContextBundle) -> impl Serialize + '_ {
    (
        bundle.status,
        &bundle.target,
        &bundle.requested_operation,
        &bundle.included_regions,
        &bundle.excluded_region_ids,
        &bundle.symbols,
        &bundle.assumptions,
        &bundle.constraints,
        &bundle.dependencies,
        &bundle.unresolved_alternatives,
        bundle.binding_handoff_ready,
        bundle.downstream_authorized,
    )
}

impl TargetContextBundle {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&replay_payload(self))
            && !self.target.is_empty()
            && !self.downstream_authorized
            && self
                .included_regions
                .iter()
                .all(|region| !region.id.is_empty())
    }
}

fn role_rank(role: RegionRole) -> u8 {
    match role {
        RegionRole::Declaration => 0,
        RegionRole::Definition => 1,
        RegionRole::Constraint => 2,
        RegionRole::Assumption => 3,
        RegionRole::Incidental => 4,
        RegionRole::Quoted => 5,
    }
}

/// Assemble a minimally relevant, deterministic context bundle.
pub fn assemble_target_context(request: &TargetContextRequest) -> TargetContextBundle {
    let mut relevant_symbols: BTreeSet<String> =
        request.target_components.iter().cloned().collect();
    relevant_symbols.extend(request.target.split_whitespace().map(str::to_string));
    let mut included = Vec::new();
    let mut excluded = Vec::new();
    let mut unresolved = Vec::new();

    let mut candidates = request.regions.clone();
    candidates.sort_by(|left, right| {
        (role_rank(left.role), &left.id).cmp(&(role_rank(right.role), &right.id))
    });
    let mut changed = true;
    while changed {
        changed = false;
        for region in &candidates {
            if matches!(region.role, RegionRole::Incidental | RegionRole::Quoted) {
                continue;
            }
            let linked = region.target_links.iter().any(|link| {
                link == &request.target
                    || request
                        .target_components
                        .iter()
                        .any(|component| component == link)
            });
            let intersects = region
                .symbols
                .iter()
                .any(|symbol| relevant_symbols.contains(symbol));
            if linked || intersects {
                for symbol in &region.symbols {
                    changed |= relevant_symbols.insert(symbol.clone());
                }
            }
        }
    }
    for region in candidates {
        if matches!(region.role, RegionRole::Incidental | RegionRole::Quoted) {
            excluded.push(region.id.clone());
            continue;
        }
        let linked = region.target_links.iter().any(|link| {
            link == &request.target
                || request
                    .target_components
                    .iter()
                    .any(|component| component == link)
        });
        let intersects = region
            .symbols
            .iter()
            .any(|symbol| relevant_symbols.contains(symbol));
        if linked || intersects {
            included.push(region);
        } else {
            excluded.push(region.id.clone());
        }
    }
    included.sort_by(|left, right| {
        (role_rank(left.role), &left.id).cmp(&(role_rank(right.role), &right.id))
    });

    let mut scopes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for region in &included {
        for symbol in &region.symbols {
            scopes
                .entry(symbol.clone())
                .or_default()
                .insert(region.scope.clone());
        }
    }
    for (symbol, symbol_scopes) in &scopes {
        if symbol_scopes.len() > 1 {
            unresolved.push(format!(
                "symbol {symbol} has multiple scopes: {symbol_scopes:?}"
            ));
        }
    }
    if included.is_empty() {
        unresolved.push("no region is justified by the requested target".into());
    }
    if request
        .regions
        .iter()
        .filter(|region| region.role == RegionRole::Constraint)
        .count()
        > 0
        && !included
            .iter()
            .any(|region| region.role == RegionRole::Constraint)
    {
        unresolved.push("constraint regions exist but none are target-relevant".into());
    }
    let has_only_excluded_regions = !request.regions.is_empty()
        && request
            .regions
            .iter()
            .all(|region| matches!(region.role, RegionRole::Incidental | RegionRole::Quoted));
    let status = if has_only_excluded_regions || request.regions.is_empty() {
        ContextStatus::Unsupported
    } else if !unresolved.is_empty() {
        ContextStatus::Ambiguous
    } else if !included.iter().any(|region| {
        matches!(
            region.role,
            RegionRole::Definition | RegionRole::Declaration | RegionRole::Constraint
        )
    }) {
        ContextStatus::Unsupported
    } else {
        ContextStatus::Complete
    };
    let assumptions = included
        .iter()
        .filter(|region| region.role == RegionRole::Assumption)
        .map(|region| region.text.clone())
        .collect();
    let constraints = included
        .iter()
        .filter(|region| region.role == RegionRole::Constraint)
        .map(|region| region.text.clone())
        .collect();
    let symbols = relevant_symbols.into_iter().collect::<Vec<_>>();
    let dependencies = included
        .iter()
        .map(|region| (region.id.clone(), region.symbols.clone()))
        .collect();
    let mut bundle = TargetContextBundle {
        status,
        target: request.target.clone(),
        requested_operation: request.requested_operation.clone(),
        included_regions: included,
        excluded_region_ids: excluded,
        symbols,
        assumptions,
        constraints,
        dependencies,
        unresolved_alternatives: unresolved,
        binding_handoff_ready: status == ContextStatus::Complete,
        replay_hash: String::new(),
        downstream_authorized: false,
    };
    let replay = digest(&replay_payload(&bundle));
    bundle.replay_hash = replay;
    bundle
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(
        id: &str,
        role: RegionRole,
        text: &str,
        symbols: &[&str],
        links: &[&str],
    ) -> ContextRegion {
        ContextRegion {
            id: id.into(),
            role,
            text: text.into(),
            symbols: symbols.iter().map(|value| (*value).into()).collect(),
            target_links: links.iter().map(|value| (*value).into()).collect(),
            scope: "root".into(),
            source_spans: vec![id.into()],
        }
    }

    #[test]
    fn includes_dependency_regions_and_excludes_quote() {
        let request = TargetContextRequest {
            target: "y".into(),
            target_components: vec!["y".into()],
            requested_operation: "compute".into(),
            regions: vec![
                region(
                    "definition",
                    RegionRole::Definition,
                    "x = 2",
                    &["x"],
                    &["y"],
                ),
                region(
                    "constraint",
                    RegionRole::Constraint,
                    "y = x + 1",
                    &["y", "x"],
                    &["y"],
                ),
                region("quote", RegionRole::Quoted, "z = unrelated", &["z"], &[]),
            ],
        };
        let bundle = assemble_target_context(&request);
        assert_eq!(bundle.status, ContextStatus::Complete);
        assert_eq!(bundle.excluded_region_ids, vec!["quote"]);
        assert!(bundle.replay_verified());
    }

    #[test]
    fn rejects_duplicate_scopes() {
        let mut left = region("left", RegionRole::Definition, "x = 1", &["x"], &["y"]);
        left.scope = "left".into();
        let mut right = region("right", RegionRole::Definition, "x = 2", &["x"], &["y"]);
        right.scope = "right".into();
        let bundle = assemble_target_context(&TargetContextRequest {
            target: "y".into(),
            target_components: vec!["y".into()],
            requested_operation: "compute".into(),
            regions: vec![left, right],
        });
        assert_eq!(bundle.status, ContextStatus::Ambiguous);
        assert!(!bundle.binding_handoff_ready);
        assert!(bundle.replay_verified());
    }
}
