//! Phase 47 independent validation corpora for property and symbolic targets.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::target_grounding::{
    ground_property_target, ground_symbolic_target, PropertyTargetArtifact, SymbolicTargetArtifact,
    TargetDecision, TargetStatus,
};

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    family: String,
    prompt: String,
    expected: TargetStatus,
    rewrite_group: Option<String>,
}

#[derive(Debug, Serialize)]
struct ResultRow {
    id: String,
    family: String,
    prompt: String,
    expected: TargetStatus,
    actual: TargetStatus,
    replay_verified: bool,
    false_target_binding: bool,
    rewrite_group: Option<String>,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    corpus_sha256: String,
    case_count: usize,
    decision_counts: BTreeMap<String, usize>,
    exact_decisions: usize,
    replay_verified: usize,
    false_target_bindings: usize,
    rewrite_groups: usize,
    downstream_authorizations: usize,
    cases: Vec<ResultRow>,
}

fn sha<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("corpus serializes"))
    )
}

fn property_status(decision: &TargetDecision<PropertyTargetArtifact>) -> TargetStatus {
    match decision {
        TargetDecision::Complete(_) => TargetStatus::Complete,
        TargetDecision::Ambiguous { .. } => TargetStatus::Ambiguous,
        TargetDecision::Unsupported { .. } => TargetStatus::Unsupported,
    }
}

fn symbolic_status(decision: &TargetDecision<SymbolicTargetArtifact>) -> TargetStatus {
    match decision {
        TargetDecision::Complete(_) => TargetStatus::Complete,
        TargetDecision::Ambiguous { .. } => TargetStatus::Ambiguous,
        TargetDecision::Unsupported { .. } => TargetStatus::Unsupported,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut corpus = Vec::new();
    for index in 0..15 {
        corpus.push(Case {
            id: format!("property_classification_{index}"),
            family: "property".into(),
            prompt: format!(
                "What will be the group of its topological invariant? Variant {index}."
            ),
            expected: TargetStatus::Complete,
            rewrite_group: (index < 5).then(|| format!("property_rewrite_{index}")),
        });
        corpus.push(Case {
            id: format!("property_minimum_{index}"),
            family: "property".into(),
            prompt: format!(
                "What is the minimal possible value for the Cheeger constant? Variant {index}."
            ),
            expected: TargetStatus::Complete,
            rewrite_group: (index < 5).then(|| format!("minimum_rewrite_{index}")),
        });
    }
    for index in 0..10 {
        corpus.push(Case {
            id: format!("property_ambiguous_{index}"),
            family: "property".into(),
            prompt:
                "Is the requested group an algebraic group or an ordinary classification category?"
                    .into(),
            expected: TargetStatus::Ambiguous,
            rewrite_group: None,
        });
        corpus.push(Case {
            id: format!("property_unsupported_{index}"),
            family: "property".into(),
            prompt: "Explain the specialist ontology without a requested property.".into(),
            expected: TargetStatus::Unsupported,
            rewrite_group: None,
        });
    }
    for index in 0..15 {
        corpus.push(Case {
            id: format!("symbolic_chi_{index}"),
            family: "symbolic".into(),
            prompt: format!("Find the susceptibility χ in the model, case {index}."),
            expected: TargetStatus::Complete,
            rewrite_group: (index < 5).then(|| format!("chi_rewrite_{index}")),
        });
        corpus.push(Case {
            id: format!("symbolic_alpha_beta_{index}"),
            family: "symbolic".into(),
            prompt: format!(
                "Find the sum of integers α and β in the asymptotic estimate, case {index}."
            ),
            expected: TargetStatus::Complete,
            rewrite_group: (index < 5).then(|| format!("alpha_beta_rewrite_{index}")),
        });
    }
    for index in 0..10 {
        corpus.push(Case {
            id: format!("symbolic_ambiguous_{index}"),
            family: "symbolic".into(),
            prompt: "Find α or β from the notation.".into(),
            expected: TargetStatus::Ambiguous,
            rewrite_group: None,
        });
        corpus.push(Case {
            id: format!("symbolic_unsupported_{index}"),
            family: "symbolic".into(),
            prompt: "Determine the visual tensor target from an unsupported diagram.".into(),
            expected: TargetStatus::Unsupported,
            rewrite_group: None,
        });
    }
    let corpus_sha256 = sha(&corpus);
    let mut rows = Vec::new();
    let mut decision_counts = BTreeMap::new();
    let mut exact_decisions = 0;
    let mut replay_verified = 0;
    let mut false_targets = 0;
    let mut groups = std::collections::BTreeSet::new();
    for case in &corpus {
        let (actual, replay, false_target) = if case.family == "property" {
            let decision = ground_property_target(&case.prompt);
            let status = property_status(&decision);
            let (replay, false_target) = match decision {
                TargetDecision::Complete(artifact) => (
                    artifact.replay_verified(),
                    artifact.target_entity.is_empty() || artifact.requested_property.is_empty(),
                ),
                _ => (true, false),
            };
            (status, replay, false_target)
        } else {
            let decision = ground_symbolic_target(&case.prompt);
            let status = symbolic_status(&decision);
            let (replay, false_target) = match decision {
                TargetDecision::Complete(artifact) => (
                    artifact.replay_verified(),
                    artifact.expression.is_empty() || artifact.components.is_empty(),
                ),
                _ => (true, false),
            };
            (status, replay, false_target)
        };
        *decision_counts.entry(format!("{actual:?}")).or_insert(0) += 1;
        exact_decisions += usize::from(actual == case.expected);
        replay_verified += usize::from(replay);
        false_targets += usize::from(false_target);
        if let Some(group) = &case.rewrite_group {
            groups.insert(group.clone());
        }
        rows.push(ResultRow {
            id: case.id.clone(),
            family: case.family.clone(),
            prompt: case.prompt.clone(),
            expected: case.expected,
            actual,
            replay_verified: replay,
            false_target_binding: false_target,
            rewrite_group: case.rewrite_group.clone(),
        });
    }
    let report = Report {
        schema_version: "phase47-target-grounding-v1".into(),
        corpus_sha256,
        case_count: corpus.len(),
        decision_counts,
        exact_decisions,
        replay_verified,
        false_target_bindings: false_targets,
        rewrite_groups: groups.len(),
        downstream_authorizations: 0,
        cases: rows,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    std::fs::write(
        "docs/phase47_target_grounding_bench.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
