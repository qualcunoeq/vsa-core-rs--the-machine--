//! Phase 22 shadow audit for normalization-contamination cases.
//!
//! The input is the full Phase 21 audit.  This binary only clusters residual
//! language/representation mechanisms; it never changes parsing or promotes a
//! normalization rule.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;

#[derive(Debug, Deserialize)]
struct AuditInput {
    input_trace_sha256: String,
    records: Vec<AuditRecord>,
}

#[derive(Debug, Deserialize)]
struct AuditRecord {
    id: Option<String>,
    category: String,
    gap: String,
    question: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Mechanism {
    SpecialistNotation,
    EmbeddedFormula,
    NestedQuestionStructure,
    CrossSentenceBinding,
    AnswerFormatConfusion,
    QuotationOrCitationStructure,
    ImplicitVariables,
    UnresolvedAbbreviation,
    DomainTerminology,
    NeedsManualReview,
}

#[derive(Debug, Serialize)]
struct MechanismRecord {
    id: Option<String>,
    category: String,
    mechanism: Mechanism,
    confidence: String,
    question: String,
}

#[derive(Debug, Serialize)]
struct Report {
    input_audit_sha256: String,
    source_trace_sha256: String,
    normalization_rows: usize,
    mechanisms: BTreeMap<Mechanism, usize>,
    samples: BTreeMap<Mechanism, Vec<String>>,
    records: Vec<MechanismRecord>,
    method: String,
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn has_any(text: &str, terms: &[&str]) -> bool {
    terms.iter().any(|term| text.contains(term))
}

fn classify(text: &str) -> (Mechanism, &'static str) {
    let lower = text.to_ascii_lowercase();
    let notation = [
        "\\math",
        "\\frac",
        "\\begin",
        "\\operatorname",
        "\\text{",
        "\\mathrm",
        "\\mathbf",
        "\\gamma",
        "\\mu",
        "\\sigma",
        "_{",
        "^{",
        "∈",
        "≤",
        "≥",
    ];
    if has_any(&lower, &notation) {
        return (Mechanism::SpecialistNotation, "high");
    }
    if lower.contains('$')
        || lower.contains("\\(")
        || lower.contains("\\[")
        || lower.contains("equation")
        || lower.contains("formula")
    {
        return (Mechanism::EmbeddedFormula, "high");
    }
    if lower.matches('?').count() > 1
        || has_any(
            &lower,
            &[
                "this is a ",
                "part 1",
                "part 2",
                "answer separately",
                "which of the following statements",
            ],
        )
    {
        return (Mechanism::NestedQuestionStructure, "medium");
    }
    if has_any(
        &lower,
        &[
            "answer with",
            "write your answer",
            "comma separated",
            "standard notation",
            "true or false",
            "output only",
            "format your answer",
            "exact number",
        ],
    ) {
        return (Mechanism::AnswerFormatConfusion, "high");
    }
    if has_any(
        &lower,
        &[
            "according to",
            "quoted",
            "citation",
            "the passage",
            "the text says",
            "the following excerpt",
            "as described above",
        ],
    ) || lower.matches('"').count() >= 2
    {
        return (Mechanism::QuotationOrCitationStructure, "medium");
    }
    if has_any(
        &lower,
        &[
            " this ",
            " that ",
            " these ",
            " those ",
            " it ",
            " they ",
            " respectively",
            "the latter",
            "the former",
            "above",
            "below",
        ],
    ) {
        return (Mechanism::CrossSentenceBinding, "medium");
    }
    if has_any(
        &lower,
        &[
            "let ",
            "denote ",
            "suppose ",
            "where ",
            "for some ",
            "such that ",
        ],
    ) && has_any(&lower, &[" x ", " y ", " z ", "variable", "unknown"])
    {
        return (Mechanism::ImplicitVariables, "medium");
    }
    if has_any(
        &lower,
        &[
            "bcr",
            "tcr",
            "gcn",
            "vc dimension",
            "sharppy",
            "uci",
            "fen",
            "pcr",
            "snv",
            "bcs",
            "stp",
        ],
    ) {
        return (Mechanism::UnresolvedAbbreviation, "medium");
    }
    if has_any(
        &lower,
        &[
            "arrhenius",
            "wasserstein",
            "k-matrix",
            "bordism",
            "homotopy",
            "polygenic",
            "rawinsonde",
            "shamir",
            "call/cc",
        ],
    ) {
        return (Mechanism::DomainTerminology, "low");
    }
    (Mechanism::NeedsManualReview, "low")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/hle_knowledge_audit_2147e9e.json".into());
    let output = env::args()
        .nth(2)
        .unwrap_or_else(|| "/tmp/hle_normalization_audit_2147e9e.json".into());
    let bytes = fs::read(&input)?;
    let input_hash = sha256(&bytes);
    let audit: AuditInput = serde_json::from_slice(&bytes)?;
    let mut mechanisms = BTreeMap::new();
    let mut samples: BTreeMap<Mechanism, Vec<String>> = BTreeMap::new();
    let mut records = Vec::new();
    for row in audit
        .records
        .into_iter()
        .filter(|row| row.gap == "apparent_gap_from_normalization")
    {
        let (mechanism, confidence) = classify(&row.question);
        *mechanisms.entry(mechanism).or_insert(0) += 1;
        let sample = samples.entry(mechanism).or_default();
        if sample.len() < 5 {
            sample.push(format!(
                "{} [{}] {}",
                row.id.as_deref().unwrap_or("no-id"),
                confidence,
                row.question.replace('\n', " ")
            ));
        }
        records.push(MechanismRecord {
            id: row.id,
            category: row.category,
            mechanism,
            confidence: confidence.into(),
            question: row.question,
        });
    }
    let report = Report {
        input_audit_sha256: input_hash,
        source_trace_sha256: audit.input_trace_sha256,
        normalization_rows: records.len(),
        mechanisms,
        samples,
        records,
        method: "deterministic shadow mechanism taxonomy; independent/manual review required before parser changes".into(),
    };
    fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mechanisms_are_deterministic() {
        assert_eq!(classify("\\frac{x}{y}").0, Mechanism::SpecialistNotation);
        assert_eq!(
            classify("write your answer as a comma separated list").0,
            Mechanism::AnswerFormatConfusion
        );
        assert_eq!(
            classify("Alice entered. What did it mean?").0,
            Mechanism::CrossSentenceBinding
        );
    }
}
