//! Produce the non-executing formalization-distance report for HLE.
//!
//! This is a diagnostic funnel.  It never invokes a solver, retrieves a
//! theorem, or changes the answer router.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use the_machine::formalization::{assess_prompt, FormalizationTrace};

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
    textual_attachment_references: usize,
    by_distance: BTreeMap<String, usize>,
    by_status: BTreeMap<String, usize>,
    by_domain: BTreeMap<String, usize>,
    obligations: BTreeMap<String, usize>,
    traces: Vec<FormalizationTrace>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let text = fs::read_to_string("data/hle.jsonl")?;
    let mut traces = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let row: HleRow = serde_json::from_str(line)?;
        traces.push(assess_prompt(
            &row.id,
            &row.question,
            &row.category,
            row.has_image,
        ));
    }
    let mut by_distance = BTreeMap::new();
    let mut by_status = BTreeMap::new();
    let mut by_domain = BTreeMap::new();
    let mut obligations = BTreeMap::new();
    for trace in &traces {
        *by_distance
            .entry(trace.modeling_distance.label().to_string())
            .or_insert(0) += 1;
        *by_status
            .entry(trace.status.label().to_string())
            .or_insert(0) += 1;
        let domain = match trace.domain {
            the_machine::math_methods::MathDomain::LinearAlgebra => "linear_algebra",
            the_machine::math_methods::MathDomain::NumberTheory => "number_theory",
            the_machine::math_methods::MathDomain::Combinatorics => "combinatorics",
            the_machine::math_methods::MathDomain::Calculus => "calculus",
            the_machine::math_methods::MathDomain::Algebra => "algebra",
            the_machine::math_methods::MathDomain::Probability => "probability",
            the_machine::math_methods::MathDomain::Geometry => "geometry",
            the_machine::math_methods::MathDomain::General => "general",
        };
        *by_domain.entry(domain.to_string()).or_insert(0) += 1;
        for obligation in &trace.obligations {
            *obligations
                .entry(obligation.label().to_string())
                .or_insert(0) += 1;
        }
    }
    let report = Report {
        scanned: traces.len(),
        textual_attachment_references: traces
            .iter()
            .filter(|trace| trace.textual_attachment_reference)
            .count(),
        by_distance,
        by_status,
        by_domain,
        obligations,
        traces,
    };
    fs::write(
        "docs/formalization_distance_20260720.json",
        serde_json::to_string_pretty(&report)?,
    )?;
    let mut markdown = String::from("# Formalization-distance report\n\nThis report is diagnostic only. It does not authorize retrieval, theorem application, or solver execution.\n\n");
    markdown.push_str(&format!(
        "Scanned **{}** questions. The benchmark flags **{}** rows with textual visual references; the remaining attachment flags are metadata only and do not prove that visual reasoning is required.\n\n",
        report.scanned, report.textual_attachment_references
    ));
    markdown.push_str("## Modeling distance\n\n| Distance | Questions |\n|---|---:|\n");
    for (distance, count) in &report.by_distance {
        markdown.push_str(&format!("| {} | {} |\n", distance, count));
    }
    markdown.push_str("\n## Formalization status\n\n| Status | Questions |\n|---|---:|\n");
    for (status, count) in &report.by_status {
        markdown.push_str(&format!("| {} | {} |\n", status, count));
    }
    markdown.push_str("\n## Modeling obligations\n\n| Obligation | Questions |\n|---|---:|\n");
    for (obligation, count) in &report.obligations {
        markdown.push_str(&format!("| {} | {} |\n", obligation, count));
    }
    markdown.push_str("\nThe runtime remains unchanged; this scan only identifies where formalization work is required.\n");
    fs::write("docs/formalization_distance_20260720.md", markdown)?;
    println!(
        "scanned={} distances={:?} obligations={:?}",
        report.scanned, report.by_distance, report.obligations
    );
    Ok(())
}
