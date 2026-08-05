//! Phase 46 diagnostic audit of the four post-parenthesis target ambiguities.
//! No parser, solver, routing, or authorization behavior is changed.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

const DATASET: &str = "data/hle.jsonl";
const RERUN: &str = "docs/phase45_hle_parenthesis_rerun.json";

#[derive(Debug, Serialize)]
struct TargetAuditCase {
    id: String,
    question_sha256: String,
    target_candidates: Vec<String>,
    requested_artifact_type: String,
    requested_operation: String,
    grammatical_evidence: Vec<String>,
    mathematical_dependencies: Vec<String>,
    local_definitions: Vec<String>,
    rejected_interpretations: Vec<String>,
    missing_distinction: String,
    recoverable_mechanism: String,
    distinguishing_evidence_required: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    dataset_sha256: String,
    rerun_sha256: String,
    audited_cases: usize,
    mechanism_counts: BTreeMap<String, usize>,
    repeated_recoverable_mechanisms: Vec<String>,
    parser_changed: bool,
    authorization_changed: bool,
    cases: Vec<TargetAuditCase>,
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset_bytes = fs::read(DATASET)?;
    let rerun_bytes = fs::read(RERUN)?;
    let rerun: Value = serde_json::from_slice(&rerun_bytes)?;
    let mut questions = BTreeMap::new();
    for line in String::from_utf8_lossy(&dataset_bytes).lines() {
        let row: Value = serde_json::from_str(line)?;
        if let (Some(id), Some(question)) = (row["id"].as_str(), row["question"].as_str()) {
            questions.insert(id.to_string(), question.to_string());
        }
    }
    let mut cases = Vec::new();
    let mut mechanism_counts = BTreeMap::new();
    for row in rerun["cases"].as_array().into_iter().flatten() {
        let id = row["id"].as_str().unwrap_or("");
        let question = questions
            .get(id)
            .ok_or_else(|| format!("missing HLE question {id}"))?;
        let (artifact, operation, grammar, dependencies, definitions, rejected, missing, mechanism, evidence) = match id {
            "66e94a88b78e263c565b17ee" => (
                "classification group",
                "classify the topological invariant",
                vec!["what will be the group of ...?"],
                vec!["T^2=-1", "P^2=-1", "codimension D=1", "tenfold classification"],
                vec!["2D free fermion model", "point defect"],
                vec!["a numeric scalar", "a matrix or equation value"],
                "whether the target is a classification label/group rather than a bound value",
                "natural_language_property_target",
                "an explicit output-type cue or typed target phrase such as classification_group",
            ),
            "67153bd7f588f3f15b038f5b" => (
                "scalar expression",
                "derive magnetic susceptibility chi",
                vec!["find chi", "the susceptibility reads ..."],
                vec!["chi", "beta", "C_l", "correlation sum", "homogeneous Ising assumptions"],
                vec!["displayed susceptibility equation", "definitions of C_l and m_0"],
                vec!["a generic scalar named chi", "an intermediate correlation term"],
                "non-ASCII target symbol and the requested derived expression are not normalized into one target",
                "non_ascii_target_symbol_normalization",
                "a notation-aware target span binding chi to the requested derived expression",
            ),
            "6717eeddd6c14a5dd1563e7c" => (
                "scalar extremal value",
                "minimize the Cheeger constant",
                vec!["what is the minimal possible value ...?"],
                vec!["h", "regular graph degree 3", "4n vertices", "Cheeger normalization"],
                vec!["definition of h", "connected 3-regular graph", "n > 100"],
                vec!["the graph itself", "an arbitrary edge-count expression"],
                "the optimization target is expressed by a natural-language superlative rather than a recognized operation",
                "natural_language_property_target",
                "an explicit target-operation binding for minimal_possible_value",
            ),
            "673a8ff77acc7cdc8c824b62" => (
                "scalar exponent combination",
                "compute alpha plus beta in an asymptotic estimate",
                vec!["find the sum of integers alpha and beta", "as X tends to infinity"],
                vec!["A(X)", "conductor", "X^a", "log^b(X)", "alpha", "beta"],
                vec!["definition of A and A(X)", "asymptotic relation"],
                vec!["the set A(X)", "the exponents separately without their sum"],
                "Greek target symbols and a compound requested expression are not normalized into one target",
                "non_ascii_target_symbol_normalization",
                "a target expression binding alpha+beta to the requested answer form",
            ),
            _ => continue,
        };
        *mechanism_counts.entry(mechanism.to_string()).or_insert(0) += 1;
        cases.push(TargetAuditCase {
            id: id.into(),
            question_sha256: sha(question.as_bytes()),
            target_candidates: row["requested_candidates"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect(),
            requested_artifact_type: artifact.into(),
            requested_operation: operation.into(),
            grammatical_evidence: grammar.into_iter().map(String::from).collect(),
            mathematical_dependencies: dependencies.into_iter().map(String::from).collect(),
            local_definitions: definitions.into_iter().map(String::from).collect(),
            rejected_interpretations: rejected.into_iter().map(String::from).collect(),
            missing_distinction: missing.into(),
            recoverable_mechanism: mechanism.into(),
            distinguishing_evidence_required: evidence.into(),
        });
    }
    cases.sort_by(|a, b| a.id.cmp(&b.id));
    let repeated_recoverable_mechanisms = mechanism_counts
        .iter()
        .filter(|(_, count)| **count >= 2)
        .map(|(mechanism, _)| mechanism.clone())
        .collect();
    let report = Report {
        schema_version: "phase46-hle-target-ambiguity-audit-v1".into(),
        dataset_sha256: sha(&dataset_bytes),
        rerun_sha256: sha(&rerun_bytes),
        audited_cases: cases.len(),
        mechanism_counts,
        repeated_recoverable_mechanisms,
        parser_changed: false,
        authorization_changed: false,
        cases,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    fs::write(
        "docs/phase46_hle_target_ambiguity_audit.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
