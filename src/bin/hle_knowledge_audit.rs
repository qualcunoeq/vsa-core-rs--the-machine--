//! Shadow taxonomy for HLE missing-knowledge diagnoses.
//!
//! This consumes the immutable per-question trace emitted by `hle_release`.
//! It does not retrieve sources, alter the registry, or authorize answers.
//! Rules are intentionally conservative and the report labels the result as
//! a review queue rather than ground-truth semantic annotation.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;

#[derive(Debug, Deserialize)]
struct TraceRow {
    id: Option<String>,
    category: String,
    question: String,
    terminal_classification: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum KnowledgeGap {
    MissingDefinitionOrTerminology,
    MissingNamedTheorem,
    MissingEquationOrScientificLaw,
    MissingEmpiricalFact,
    MissingTaxonomicFact,
    MissingHistoricalOrTextualKnowledge,
    MissingSpecialistConvention,
    DerivationAfterFactualRetrieval,
    ApparentGapFromNormalization,
    NeedsManualReview,
}

#[derive(Debug, Serialize)]
struct AuditReport {
    input_trace_sha256: String,
    scanned_trace_rows: usize,
    missing_knowledge_rows: usize,
    classifications: BTreeMap<KnowledgeGap, usize>,
    samples: BTreeMap<KnowledgeGap, Vec<String>>,
    method: String,
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn has_any(text: &str, terms: &[&str]) -> bool {
    terms.iter().any(|term| text.contains(term))
}

fn classify(row: &TraceRow) -> (KnowledgeGap, &'static str) {
    let text = row.question.to_ascii_lowercase();
    let category = row.category.to_ascii_lowercase();

    if has_any(&text, &["notation", "convention", "standard notation", "syntax", "protocol", "uci", "fen", "format", "abbreviation"]) {
        return (KnowledgeGap::MissingSpecialistConvention, "high");
    }
    if has_any(&text, &["theorem", "lemma", "corollary", "axiom", "conjecture", "principle of"]) {
        return (KnowledgeGap::MissingNamedTheorem, "high");
    }
    if has_any(&text, &["equation", "formula", "scientific law", "law of", "derive", "differential", "integral", "calculate", "compute", "what is the value"]) && has_any(&text, &["=", "formula", "equation", "derive", "calculate", "compute", "value"]) {
        return (KnowledgeGap::DerivationAfterFactualRetrieval, "medium");
    }
    if has_any(&text, &["law", "equation", "formula", "reaction", "mechanism", "model", "constant", "rate of"]) {
        return (KnowledgeGap::MissingEquationOrScientificLaw, "medium");
    }
    if has_any(&text, &["species", "genus", "family", "order", "phylum", "taxonomy", "classification", "subclass", "diagnosis", "syndrome", "organism"]) || has_any(&category, &["biology", "medicine", "chemistry"]) && has_any(&text, &["which", "what type", "class", "kind", "called"]) {
        return (KnowledgeGap::MissingTaxonomicFact, "medium");
    }
    if has_any(&text, &["author", "book", "novel", "poem", "play", "album", "song", "historical", "war", "century", "president", "wrote", "published", "episode", "film", "movie"]) || has_any(&category, &["humanities", "social science"]) {
        return (KnowledgeGap::MissingHistoricalOrTextualKnowledge, "medium");
    }
    if has_any(&text, &["definition", "defined as", "meaning of", "refers to", "what does", "what is meant by", "term "]) {
        return (KnowledgeGap::MissingDefinitionOrTerminology, "medium");
    }
    if text.contains("$") || text.contains("\\(") || text.contains("\\[") || text.matches('{').count() != text.matches('}').count() || text.matches('(').count() != text.matches(')').count() {
        return (KnowledgeGap::ApparentGapFromNormalization, "low");
    }
    if has_any(&text, &["what is", "which", "who", "when", "where", "name", "identify", "how many", "how much", "what does"]) {
        return (KnowledgeGap::MissingEmpiricalFact, "low");
    }
    (KnowledgeGap::NeedsManualReview, "low")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/hle_release_candidate_2147e9e.traces.jsonl".into());
    let output = env::args()
        .nth(2)
        .unwrap_or_else(|| "/tmp/hle_knowledge_audit_2147e9e.json".into());
    let bytes = fs::read(&input)?;
    let mut rows = Vec::new();
    for line in String::from_utf8(bytes.clone())?.lines() {
        if !line.trim().is_empty() {
            rows.push(serde_json::from_str::<TraceRow>(line)?);
        }
    }
    let mut classifications = BTreeMap::new();
    let mut samples: BTreeMap<KnowledgeGap, Vec<String>> = BTreeMap::new();
    let missing: Vec<_> = rows
        .iter()
        .filter(|row| row.terminal_classification == "missing_factual_knowledge")
        .collect();
    for row in &missing {
        let (gap, confidence) = classify(row);
        *classifications.entry(gap).or_insert(0) += 1;
        let sample = samples.entry(gap).or_default();
        if sample.len() < 5 {
            sample.push(format!("{} [{}] {}", row.id.as_deref().unwrap_or("no-id"), confidence, row.question.replace('\n', " ")));
        }
    }
    let report = AuditReport {
        input_trace_sha256: hash(&bytes),
        scanned_trace_rows: rows.len(),
        missing_knowledge_rows: missing.len(),
        classifications,
        samples,
        method: "deterministic lexical shadow taxonomy; manual review required before acquisition".into(),
    };
    fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(question: &str, category: &str) -> TraceRow {
        TraceRow {
            id: Some("test".into()),
            category: category.into(),
            question: question.into(),
            terminal_classification: "missing_factual_knowledge".into(),
        }
    }

    #[test]
    fn taxonomy_is_deterministic_and_keeps_normalization_separate() {
        assert_eq!(classify(&row("Which theorem applies?", "Math")).0, KnowledgeGap::MissingNamedTheorem);
        assert_eq!(classify(&row("Malformed expression with an unmatched brace {", "Math")).0, KnowledgeGap::ApparentGapFromNormalization);
        assert_eq!(classify(&row("Which species belongs to this genus?", "Biology/Medicine")).0, KnowledgeGap::MissingTaxonomicFact);
    }
}
