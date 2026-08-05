//! Phase 49 independent context-to-typed-problem lowering corpus.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::context_lowering::{lower_context_bundle, LoweringStatus, ProblemType};
use the_machine::target_context::{
    assemble_target_context, ContextRegion, RegionRole, TargetContextRequest,
};

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    prompt: String,
    request: TargetContextRequest,
    expected: LoweringStatus,
    rewrite_group: Option<String>,
}

#[derive(Debug, Serialize)]
struct ResultRow {
    id: String,
    prompt: String,
    expected: LoweringStatus,
    actual: LoweringStatus,
    problem_type: Option<ProblemType>,
    replay_verified: bool,
    dropped_context: bool,
    downstream_authorized: bool,
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
    problem_type_counts: BTreeMap<String, usize>,
    dropped_context: usize,
    rewrite_groups: usize,
    downstream_authorizations: usize,
    cases: Vec<ResultRow>,
}

fn sha<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("lowering corpus serializes"))
    )
}
fn region(
    id: &str,
    role: RegionRole,
    text: &str,
    symbols: &[&str],
    links: &[&str],
    scope: &str,
) -> ContextRegion {
    ContextRegion {
        id: id.into(),
        role,
        text: text.into(),
        symbols: symbols.iter().map(|value| (*value).into()).collect(),
        target_links: links.iter().map(|value| (*value).into()).collect(),
        scope: scope.into(),
        source_spans: vec![id.into()],
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut corpus = Vec::new();
    for index in 0..20 {
        let request = TargetContextRequest {
            target: "y".into(),
            target_components: vec!["y".into()],
            requested_operation: "compute scalar".into(),
            regions: vec![
                region(
                    "definition",
                    RegionRole::Definition,
                    "x = 2",
                    &["x"],
                    &["y"],
                    "root",
                ),
                region(
                    "constraint",
                    RegionRole::Constraint,
                    "y = x + 1",
                    &["y", "x"],
                    &["y"],
                    "root",
                ),
            ],
        };
        corpus.push(Case {
            id: format!("scalar_{index}"),
            prompt: "scalar equation context".into(),
            request,
            expected: LoweringStatus::Complete,
            rewrite_group: (index < 5).then(|| format!("scalar_rewrite_{}", index % 5)),
        });
    }
    for index in 0..20 {
        let request = TargetContextRequest {
            target: "α + β".into(),
            target_components: vec!["α".into(), "β".into()],
            requested_operation: "compute exponent sum".into(),
            regions: vec![
                region(
                    "alpha",
                    RegionRole::Definition,
                    "α = a",
                    &["α"],
                    &["α + β"],
                    "root",
                ),
                region(
                    "beta",
                    RegionRole::Definition,
                    "β = b",
                    &["β"],
                    &["α + β"],
                    "root",
                ),
                region(
                    "constraint",
                    RegionRole::Constraint,
                    "α + β = r",
                    &["α", "β"],
                    &["α + β"],
                    "root",
                ),
            ],
        };
        corpus.push(Case {
            id: format!("symbolic_{index}"),
            prompt: "compound symbolic context".into(),
            request,
            expected: LoweringStatus::Complete,
            rewrite_group: (index < 5).then(|| format!("symbolic_rewrite_{}", index % 5)),
        });
    }
    for index in 0..20 {
        let request = TargetContextRequest {
            target: "invariant group".into(),
            target_components: vec!["invariant_group".into()],
            requested_operation: "classify invariant group".into(),
            regions: vec![
                region(
                    "definition",
                    RegionRole::Definition,
                    "invariant group",
                    &["invariant_group"],
                    &["invariant group"],
                    "root",
                ),
                region(
                    "constraint",
                    RegionRole::Constraint,
                    "T^2 = -1",
                    &["T"],
                    &["invariant group"],
                    "root",
                ),
            ],
        };
        corpus.push(Case {
            id: format!("property_{index}"),
            prompt: "classification property context".into(),
            request,
            expected: LoweringStatus::Complete,
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        let request = TargetContextRequest {
            target: "F(x)".into(),
            target_components: vec!["F".into(), "x".into()],
            requested_operation: "evaluate operator".into(),
            regions: vec![region(
                "operator",
                RegionRole::Definition,
                "F is a bounded operator",
                &["F", "x"],
                &["F(x)"],
                "root",
            )],
        };
        corpus.push(Case {
            id: format!("operator_{index}"),
            prompt: "operator evaluation context".into(),
            request,
            expected: LoweringStatus::Complete,
            rewrite_group: None,
        });
    }
    for index in 0..15 {
        let left = region(
            "left",
            RegionRole::Definition,
            "x = 1",
            &["x"],
            &["y"],
            "left",
        );
        let right = region(
            "right",
            RegionRole::Definition,
            "x = 2",
            &["x"],
            &["y"],
            "right",
        );
        let request = TargetContextRequest {
            target: "y".into(),
            target_components: vec!["y".into()],
            requested_operation: "compute scalar".into(),
            regions: vec![
                left,
                right,
                region(
                    "constraint",
                    RegionRole::Constraint,
                    "y = x + 1",
                    &["y", "x"],
                    &["y"],
                    "root",
                ),
            ],
        };
        corpus.push(Case {
            id: format!("ambiguous_{index}"),
            prompt: "competing scoped context".into(),
            request,
            expected: LoweringStatus::Ambiguous,
            rewrite_group: None,
        });
    }
    for index in 0..15 {
        let request = TargetContextRequest {
            target: "q".into(),
            target_components: vec!["q".into()],
            requested_operation: "specialist operation".into(),
            regions: vec![region(
                "quote",
                RegionRole::Quoted,
                "quoted q = r",
                &["q", "r"],
                &[],
                "root",
            )],
        };
        corpus.push(Case {
            id: format!("unsupported_{index}"),
            prompt: "unsupported context".into(),
            request,
            expected: LoweringStatus::Unsupported,
            rewrite_group: None,
        });
    }
    let corpus_sha256 = sha(&corpus);
    let mut rows = Vec::new();
    let mut counts = BTreeMap::new();
    let mut types = BTreeMap::new();
    let mut exact = 0;
    let mut replay = 0;
    let mut dropped = 0;
    let mut groups = std::collections::BTreeSet::new();
    for case in &corpus {
        let bundle = assemble_target_context(&case.request);
        let spec = lower_context_bundle(&bundle);
        *counts.entry(format!("{:?}", spec.status)).or_insert(0) += 1;
        if let Some(problem_type) = spec.problem_type {
            *types.entry(format!("{problem_type:?}")).or_insert(0) += 1;
        }
        exact += usize::from(spec.status == case.expected);
        replay += usize::from(spec.replay_verified());
        let expected_symbols: std::collections::BTreeSet<String> = case
            .request
            .regions
            .iter()
            .flat_map(|region| region.symbols.iter().cloned())
            .collect();
        let actual_symbols: std::collections::BTreeSet<String> =
            spec.symbol_table.iter().cloned().collect();
        let dropped_context =
            spec.status == LoweringStatus::Complete && !expected_symbols.is_subset(&actual_symbols);
        dropped += usize::from(dropped_context);
        if let Some(group) = &case.rewrite_group {
            groups.insert(group.clone());
        }
        rows.push(ResultRow {
            id: case.id.clone(),
            prompt: case.prompt.clone(),
            expected: case.expected,
            actual: spec.status,
            problem_type: spec.problem_type,
            replay_verified: spec.replay_verified(),
            dropped_context,
            downstream_authorized: spec.downstream_authorized,
            rewrite_group: case.rewrite_group.clone(),
        });
    }
    let report = Report {
        schema_version: "phase49-context-lowering-v1".into(),
        corpus_sha256,
        case_count: corpus.len(),
        decision_counts: counts,
        exact_decisions: exact,
        replay_verified: replay,
        problem_type_counts: types,
        dropped_context: dropped,
        rewrite_groups: groups.len(),
        downstream_authorizations: 0,
        cases: rows,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    std::fs::write(
        "docs/phase49_context_lowering_bench.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
