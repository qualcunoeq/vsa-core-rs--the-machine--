//! Phase 44 independent cross-domain pressure corpus for EquationProblemBindingV1.
//! The corpus exercises binding only; no solver is called and no case can authorize.

use serde::Serialize;
use serde_json::to_vec_pretty;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::equation_problem_binding::{bind_equation_problem, BindingStatus};

#[derive(Debug, Serialize, Clone)]
struct Case {
    id: String,
    domain: String,
    prompt: String,
    expected: BindingStatus,
    rewrite_group: Option<String>,
}

#[derive(Debug, Serialize)]
struct ResultRow {
    id: String,
    domain: String,
    prompt: String,
    rewrite_group: Option<String>,
    expected: BindingStatus,
    actual: BindingStatus,
    replay_verified: bool,
    downstream_authorized: bool,
    target: Option<String>,
    symbol_count: usize,
    constraint_count: usize,
    reason: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    corpus_sha256: String,
    case_count: usize,
    decision_counts: BTreeMap<String, usize>,
    exact_decisions: usize,
    replay_verified: usize,
    false_symbol_or_target_bindings: usize,
    assumption_propagation_cases: usize,
    coupled_constraint_cases: usize,
    rewrite_groups: usize,
    downstream_authorizations: usize,
    cases: Vec<ResultRow>,
}

fn hash<T: serde::Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("serialize"))
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let domains = [
        "algebra",
        "regression",
        "probability",
        "recurrence",
        "mechanics",
        "matrix",
        "functions",
    ];
    let mut corpus = Vec::new();
    for (index, domain) in domains.iter().enumerate() {
        for offset in 0..10 {
            let prompt = match *domain {
                "algebra" => format!("Let x = {}. Constraint z = x + 1. Solve for z.", offset + 2),
                "regression" => format!("Let beta_0 = 1 and beta_1 = {}. Constraint y = beta_0 + beta_1*x. Solve for y.", offset + 1),
                "probability" => format!("Let p = 0.{}. Constraint q = 1 - p. Solve for q.", offset + 1),
                "recurrence" => format!("Let a_i = i + {} and i = 1..n. Constraint y = a_i. Solve for y.", offset + 1),
                "mechanics" => format!("Let m = {} kg. Assuming m > 0. Constraint F = m*a. Solve for F.", offset + 1),
                "matrix" => format!("Let A = [[{}, 0], [0, 1]]. Constraint d = det(A). Solve for d.", offset + 1),
                "functions" => format!("Let f: R -> R. Constraint y = f(x). Solve for y at x = {}.", offset + 1),
                _ => unreachable!(),
            };
            corpus.push(Case {
                id: format!("supported_{index}_{offset}"),
                domain: (*domain).into(),
                prompt,
                expected: BindingStatus::Complete,
                rewrite_group: (offset < 5).then(|| format!("rewrite_{index}_{offset}")),
            });
        }
    }
    let ambiguous = [
        "Find x and y from x + y = 3.",
        "Let x = 1 in scope A and x = 2 in scope B. Find x.",
        "Let a_i = 2*i. Find a_n.",
        "Let f(x) = x^2. Evaluate f(3).",
        "The measurement x = 3 was observed; find x.",
        "Either x + y = 3 or x - y = 1. Solve for x.",
        "Find the unknown from x + y = 3.",
        "Assuming the usual convention, calculate q.",
        "Several constraint systems are possible; solve for x.",
        "Let x = 1. Find x and y.",
    ];
    for (i, prompt) in ambiguous.iter().cycle().take(30).enumerate() {
        corpus.push(Case {
            id: format!("ambiguous_{i}"),
            domain: domains[i % domains.len()].into(),
            prompt: (*prompt).into(),
            expected: BindingStatus::Ambiguous,
            rewrite_group: (i < 10).then(|| format!("ambiguous_rewrite_{}", i % 5)),
        });
    }
    let unsupported = [
        "Solve the PDE on an infinite-dimensional function space.",
        "Use the visual diagram to determine the operator.",
        "Apply quantum field theory to find the unknown.",
        "Interpret the unknown convention and calculate x.",
        "Unsupported representation: determine the tensor invariant from an omitted specialist representation.",
    ];
    for (i, prompt) in unsupported.iter().cycle().take(20).enumerate() {
        corpus.push(Case {
            id: format!("unsupported_{i}"),
            domain: "unsupported".into(),
            prompt: (*prompt).into(),
            expected: BindingStatus::Unsupported,
            rewrite_group: None,
        });
    }

    let corpus_sha256 = hash(&corpus);
    let mut rows = Vec::new();
    let mut decision_counts = BTreeMap::new();
    let mut exact_decisions = 0;
    let mut replay_verified = 0;
    let mut false_bindings = 0;
    let mut assumptions = 0;
    let mut coupled = 0;
    let mut groups = std::collections::BTreeSet::new();
    let mut authorizations = 0;
    for case in &corpus {
        let result = bind_equation_problem(&case.prompt);
        let actual = result.status;
        *decision_counts.entry(format!("{:?}", actual)).or_insert(0) += 1;
        exact_decisions += usize::from(actual == case.expected);
        replay_verified += usize::from(result.replay_verified());
        authorizations += usize::from(result.downstream_authorized);
        assumptions += usize::from(!result.assumptions.is_empty());
        coupled += usize::from(result.constraints.len() > 1);
        if let Some(group) = &case.rewrite_group {
            groups.insert(group.clone());
        }
        if actual == BindingStatus::Complete && result.requested_unknown.selected.is_none() {
            false_bindings += 1;
        }
        rows.push(ResultRow {
            id: case.id.clone(),
            domain: case.domain.clone(),
            prompt: case.prompt.clone(),
            rewrite_group: case.rewrite_group.clone(),
            expected: case.expected,
            actual,
            replay_verified: result.replay_verified(),
            downstream_authorized: result.downstream_authorized,
            target: result.requested_unknown.selected.clone(),
            symbol_count: result.symbols.len(),
            constraint_count: result.constraints.len(),
            reason: result.reason.clone(),
        });
    }
    let report = Report {
        schema_version: "phase44-equation-problem-binding-v1".into(),
        corpus_sha256,
        case_count: corpus.len(),
        decision_counts,
        exact_decisions,
        replay_verified,
        false_symbol_or_target_bindings: false_bindings,
        assumption_propagation_cases: assumptions,
        coupled_constraint_cases: coupled,
        rewrite_groups: groups.len(),
        downstream_authorizations: authorizations,
        cases: rows,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    std::fs::write(
        "docs/phase44_equation_problem_binding_bench.json",
        to_vec_pretty(&report)?,
    )?;
    Ok(())
}
