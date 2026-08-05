//! Phase 48 independent target-context assembly corpus.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::target_context::{
    assemble_target_context, ContextRegion, ContextStatus, RegionRole, TargetContextRequest,
};

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    prompt: String,
    request: TargetContextRequest,
    expected: ContextStatus,
    rewrite_group: Option<String>,
}

#[derive(Debug, Serialize)]
struct ResultRow {
    id: String,
    prompt: String,
    expected: ContextStatus,
    actual: ContextStatus,
    replay_verified: bool,
    binding_handoff_ready: bool,
    included_region_ids: Vec<String>,
    excluded_region_ids: Vec<String>,
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
    correct_inclusion_exclusion: usize,
    rewrite_groups: usize,
    binding_handoff_ready: usize,
    downstream_authorizations: usize,
    cases: Vec<ResultRow>,
}

fn sha<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("context corpus serializes"))
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
    for index in 0..30 {
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
                region(
                    "assumption",
                    RegionRole::Assumption,
                    "x > 0",
                    &["x"],
                    &["x"],
                    "root",
                ),
                region(
                    "incidental",
                    RegionRole::Incidental,
                    "q = unrelated",
                    &["q"],
                    &[],
                    "root",
                ),
                region(
                    "quote",
                    RegionRole::Quoted,
                    "quoted = formula",
                    &["quoted"],
                    &[],
                    "root",
                ),
            ],
        };
        corpus.push(Case {
            id: format!("supported_{index}"),
            prompt: format!(
                "Target y with separated definitions, constraints, and assumptions ({index})"
            ),
            request,
            expected: ContextStatus::Complete,
            rewrite_group: (index < 10).then(|| format!("supported_rewrite_{}", index % 5)),
        });
    }
    for index in 0..30 {
        let mut left = region(
            "left",
            RegionRole::Definition,
            "x = 1",
            &["x"],
            &["y"],
            "left",
        );
        let mut right = region(
            "right",
            RegionRole::Definition,
            "x = 2",
            &["x"],
            &["y"],
            "right",
        );
        if index % 2 == 0 {
            std::mem::swap(&mut left, &mut right);
        }
        let request = TargetContextRequest {
            target: "y".into(),
            target_components: vec!["y".into()],
            requested_operation: "compute".into(),
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
            prompt: format!("Target y with competing scopes ({index})"),
            request,
            expected: ContextStatus::Ambiguous,
            rewrite_group: None,
        });
    }
    for index in 0..30 {
        let request = TargetContextRequest {
            target: "y".into(),
            target_components: vec!["y".into()],
            requested_operation: "compute".into(),
            regions: vec![
                region(
                    "quote",
                    RegionRole::Quoted,
                    "quoted y = x + 1",
                    &["y", "x"],
                    &[],
                    "root",
                ),
                region(
                    "incidental",
                    RegionRole::Incidental,
                    "unrelated z = 4",
                    &["z"],
                    &[],
                    "root",
                ),
            ],
        };
        corpus.push(Case {
            id: format!("unsupported_{index}"),
            prompt: format!("Target y with no asserted context ({index})"),
            request,
            expected: ContextStatus::Unsupported,
            rewrite_group: None,
        });
    }
    let corpus_sha256 = sha(&corpus);
    let mut rows = Vec::new();
    let mut counts = BTreeMap::new();
    let mut exact = 0;
    let mut replay = 0;
    let mut inclusion = 0;
    let mut groups = std::collections::BTreeSet::new();
    let mut handoff = 0;
    for case in &corpus {
        let bundle = assemble_target_context(&case.request);
        *counts.entry(format!("{:?}", bundle.status)).or_insert(0) += 1;
        exact += usize::from(bundle.status == case.expected);
        replay += usize::from(bundle.replay_verified());
        handoff += usize::from(bundle.binding_handoff_ready);
        let expected_ids: std::collections::BTreeSet<String> = match case.expected {
            ContextStatus::Complete => ["definition", "constraint", "assumption"]
                .into_iter()
                .map(String::from)
                .collect(),
            ContextStatus::Ambiguous => ["left", "right", "constraint"]
                .into_iter()
                .map(String::from)
                .collect(),
            ContextStatus::Unsupported => {
                BTreeMap::<String, String>::new().keys().cloned().collect()
            }
        };
        let actual_ids: std::collections::BTreeSet<String> = bundle
            .included_regions
            .iter()
            .map(|region| region.id.clone())
            .collect();
        inclusion += usize::from(actual_ids == expected_ids);
        if let Some(group) = &case.rewrite_group {
            groups.insert(group.clone());
        }
        rows.push(ResultRow {
            id: case.id.clone(),
            prompt: case.prompt.clone(),
            expected: case.expected,
            actual: bundle.status,
            replay_verified: bundle.replay_verified(),
            binding_handoff_ready: bundle.binding_handoff_ready,
            included_region_ids: actual_ids.into_iter().collect(),
            excluded_region_ids: bundle.excluded_region_ids.clone(),
            rewrite_group: case.rewrite_group.clone(),
        });
    }
    let report = Report {
        schema_version: "phase48-target-context-v1".into(),
        corpus_sha256,
        case_count: corpus.len(),
        decision_counts: counts,
        exact_decisions: exact,
        replay_verified: replay,
        correct_inclusion_exclusion: inclusion,
        rewrite_groups: groups.len(),
        binding_handoff_ready: handoff,
        downstream_authorizations: 0,
        cases: rows,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    std::fs::write(
        "docs/phase48_target_context_bench.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
