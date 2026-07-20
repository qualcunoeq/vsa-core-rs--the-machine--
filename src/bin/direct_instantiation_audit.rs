//! Audit the formalization report's direct-instantiation predictions.
//!
//! This is a second diagnostic gate.  It classifies the supplied object and
//! requested use, then reports representation readiness.  It never executes a
//! definition, theorem, or solver.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use the_machine::formalization::{
    assess_direct_instantiation, assess_prompt, DirectInstantiationAssessment, FormalizationTrace,
    InstantiationTargetKind, ModelingDistance, RepresentationReadiness, SuppliedObjectKind,
};

#[derive(Debug, Deserialize)]
struct HleRow {
    id: String,
    question: String,
    category: String,
    #[serde(default)]
    has_image: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    scanned: usize,
    direct_predictions: usize,
    by_object: BTreeMap<String, usize>,
    by_target: BTreeMap<String, usize>,
    by_readiness: BTreeMap<String, usize>,
    missing_representation: BTreeMap<String, usize>,
    authorization_blockers: BTreeMap<String, usize>,
    ids_by_readiness: BTreeMap<String, Vec<String>>,
    ids_by_object: BTreeMap<String, Vec<String>>,
    ids_by_target: BTreeMap<String, Vec<String>>,
    assessments: Vec<DirectInstantiationAssessment>,
}

fn object_label(value: SuppliedObjectKind) -> &'static str {
    value.label()
}

fn target_label(value: InstantiationTargetKind) -> &'static str {
    value.label()
}

fn readiness_label(value: RepresentationReadiness) -> &'static str {
    value.label()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let text = fs::read_to_string("data/hle.jsonl")?;
    let mut traces: Vec<FormalizationTrace> = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let row: HleRow = serde_json::from_str(line)?;
        traces.push(assess_prompt(
            &row.id,
            &row.question,
            &row.category,
            row.has_image,
        ));
    }
    let candidates = traces
        .iter()
        .filter(|trace| trace.modeling_distance == ModelingDistance::DirectInstantiation)
        .map(assess_direct_instantiation)
        .collect::<Vec<_>>();
    let mut by_object = BTreeMap::new();
    let mut by_target = BTreeMap::new();
    let mut by_readiness = BTreeMap::new();
    let mut missing_representation = BTreeMap::new();
    let mut authorization_blockers = BTreeMap::new();
    let mut ids_by_readiness: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut ids_by_object: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut ids_by_target: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for assessment in &candidates {
        let object = object_label(assessment.supplied_object).to_string();
        let target = target_label(assessment.target).to_string();
        let readiness = readiness_label(assessment.readiness).to_string();
        *by_object.entry(object.clone()).or_insert(0) += 1;
        *by_target.entry(target.clone()).or_insert(0) += 1;
        *by_readiness.entry(readiness.clone()).or_insert(0) += 1;
        ids_by_object
            .entry(object)
            .or_default()
            .push(assessment.question_id.clone());
        ids_by_target
            .entry(target)
            .or_default()
            .push(assessment.question_id.clone());
        ids_by_readiness
            .entry(readiness)
            .or_default()
            .push(assessment.question_id.clone());
        for missing in &assessment.missing_representation {
            *missing_representation.entry(missing.clone()).or_insert(0) += 1;
        }
        for blocker in &assessment.authorization_blockers {
            *authorization_blockers.entry(blocker.clone()).or_insert(0) += 1;
        }
    }
    let report = Report {
        scanned: traces.len(),
        direct_predictions: candidates.len(),
        by_object,
        by_target,
        by_readiness,
        missing_representation,
        authorization_blockers,
        ids_by_readiness,
        ids_by_object,
        ids_by_target,
        assessments: candidates,
    };
    fs::write(
        "docs/direct_instantiation_audit_20260720.json",
        serde_json::to_string_pretty(&report)?,
    )?;
    let mut markdown = String::from(
        "# Direct-instantiation audit\n\nThis report is diagnostic only. It does not authorize definition application, theorem use, or solver execution.\n\n",
    );
    markdown.push_str(&format!(
        "Scanned **{}** questions; **{}** were classified as direct-instantiation-shaped.\n\n",
        report.scanned, report.direct_predictions
    ));
    markdown.push_str("## Representation readiness\n\n| Readiness | Questions |\n|---|---:|\n");
    for (key, value) in &report.by_readiness {
        markdown.push_str(&format!("| {} | {} |\n", key, value));
    }
    markdown.push_str("\n## Supplied objects\n\n| Object | Questions |\n|---|---:|\n");
    for (key, value) in &report.by_object {
        markdown.push_str(&format!("| {} | {} |\n", key, value));
    }
    markdown.push_str("\n## Requested uses\n\n| Target | Questions |\n|---|---:|\n");
    for (key, value) in &report.by_target {
        markdown.push_str(&format!("| {} | {} |\n", key, value));
    }
    markdown.push_str("\n## Missing representation\n\n| Gap | Questions |\n|---|---:|\n");
    for (key, value) in &report.missing_representation {
        markdown.push_str(&format!("| {} | {} |\n", key, value));
    }
    markdown.push_str("\n## Authorization blockers\n\n| Blocker | Questions |\n|---|---:|\n");
    for (key, value) in &report.authorization_blockers {
        markdown.push_str(&format!("| {} | {} |\n", key, value));
    }
    markdown.push_str("\n## Supporting question IDs by readiness\n\n");
    for (key, ids) in &report.ids_by_readiness {
        markdown.push_str(&format!(
            "- **{}** ({}): {}\n",
            key,
            ids.len(),
            ids.join(", ")
        ));
    }
    markdown.push_str(
        "\nThe JSON report also contains exact IDs for every supplied-object and target bucket.\n",
    );
    markdown.push_str("\nEvery successful-looking assessment still requires manual review and an independent verifier before any runtime route can be enabled.\n");
    fs::write("docs/direct_instantiation_audit_20260720.md", markdown)?;
    println!(
        "scanned={} direct_predictions={} readiness={:?} objects={:?} targets={:?}",
        report.scanned,
        report.direct_predictions,
        report.by_readiness,
        report.by_object,
        report.by_target
    );
    Ok(())
}
