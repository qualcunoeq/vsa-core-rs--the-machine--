//! Phase 44 follow-up: evidence audit for the ten ambiguous frozen HLE cases.
//! This is intentionally diagnostic-only and does not alter the binder.

use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

const DATASET: &str = "data/hle.jsonl";
const SHADOW_REPORT: &str = "docs/phase44_hle_equation_problem_binding_shadow.json";

#[derive(Debug, Serialize)]
struct AuditCase {
    id: String,
    question_sha256: String,
    mechanism: String,
    evidence: Vec<String>,
    recoverable_without_new_domain_method: bool,
    proposed_next_evidence: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    dataset_sha256: String,
    shadow_report_sha256: String,
    audited_cases: usize,
    mechanism_counts: BTreeMap<String, usize>,
    repeated_recoverable_mechanisms: Vec<String>,
    binder_changed: bool,
    cases: Vec<AuditCase>,
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset_bytes = fs::read(DATASET)?;
    let shadow_bytes = fs::read(SHADOW_REPORT)?;
    let shadow: Value = serde_json::from_slice(&shadow_bytes)?;
    let function_like = Regex::new(r"\b[A-Za-z][A-Za-z0-9_]*\s*\([^)]*\)")?;
    let mut cases = Vec::new();
    let mut mechanism_counts = BTreeMap::new();
    for row in shadow["cases"].as_array().into_iter().flatten() {
        if row["binding_status"].as_str() != Some("ambiguous") {
            continue;
        }
        let id = row["id"].as_str().unwrap_or("");
        let question = String::from_utf8_lossy(&dataset_bytes)
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .find(|entry| entry["id"].as_str() == Some(id))
            .and_then(|entry| entry["question"].as_str().map(str::to_string))
            .unwrap_or_default();
        let lower = question.to_ascii_lowercase();
        let hits: Vec<String> = function_like
            .find_iter(&question)
            .map(|m| m.as_str().to_string())
            .take(8)
            .collect();
        let (mechanism, evidence, recoverable, next) = if !lower.contains("function")
            && !lower.contains("pde")
            && !lower.contains("partial")
            && !lower.contains("schur")
            && !lower.contains("traffic flow")
            && !lower.contains("u(t")
            && !lower.contains("f(x)")
        {
            (
                "overbroad_parenthesis_function_detection".to_string(),
                vec![
                    "binder saw callable-looking parentheses in prose or operator notation".into(),
                    format!("candidate spans: {}", hits.join(" | ")),
                ],
                true,
                "require an explicit function declaration or typed callable use before applying the function-domain boundary".into(),
            )
        } else {
            (
                "domain_dependent_function_or_operator_semantics".to_string(),
                vec![
                    "function-like notation is materially part of the problem".into(),
                    format!("candidate spans: {}", hits.join(" | ")),
                ],
                false,
                "obtain explicit domain/codomain or operator semantics from local evidence; otherwise preserve ambiguity".into(),
            )
        };
        *mechanism_counts.entry(mechanism.clone()).or_insert(0) += 1;
        cases.push(AuditCase {
            id: id.into(),
            question_sha256: sha(question.as_bytes()),
            mechanism,
            evidence,
            recoverable_without_new_domain_method: recoverable,
            proposed_next_evidence: next,
        });
    }
    cases.sort_by(|a, b| a.id.cmp(&b.id));
    let repeated_recoverable_mechanisms = mechanism_counts
        .iter()
        .filter(|(mechanism, count)| **count >= 2 && mechanism.contains("overbroad"))
        .map(|(mechanism, _)| mechanism.clone())
        .collect();
    let report = Report {
        schema_version: "phase44-equation-binding-ambiguity-audit-v1".into(),
        dataset_sha256: sha(&dataset_bytes),
        shadow_report_sha256: sha(&shadow_bytes),
        audited_cases: cases.len(),
        mechanism_counts,
        repeated_recoverable_mechanisms,
        binder_changed: false,
        cases,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    fs::write(
        "docs/phase44_equation_binding_ambiguity_audit.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
