//! Curated, provenance-preserving knowledge and a conservative entailment gate.
//!
//! Extracted formulas remain useful retrieval candidates, but cannot become
//! executable evidence until their provenance and applicability are reviewed.

use crate::physics::PhysicsLaw;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum KnowledgeQuality {
    Candidate,
    Curated,
    Trusted,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeRecord {
    pub id: String,
    pub statement: String,
    pub formula: Option<String>,
    pub source: String,
    pub domain: String,
    pub variables: Vec<String>,
    pub assumptions: Vec<String>,
    pub units: HashMap<String, String>,
    pub quality: KnowledgeQuality,
    /// Concrete, anchored prompts that this record is allowed to establish.
    /// These are regression probes for the retrieval plus entailment gate;
    /// they are not training examples or a source of inferred synonyms.
    #[serde(default)]
    pub entailment_examples: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntailmentVerdict {
    Entailed,
    CandidateOnly,
    DomainMismatch,
    MissingSource,
    ConditionsNotEstablished,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CuratedKnowledgeStore {
    records: Vec<KnowledgeRecord>,
    by_id: HashMap<String, usize>,
}

impl CuratedKnowledgeStore {
    /// Build a store from a deliberately small evidence pack.  Unlike the
    /// formula-cache constructor, this preserves the record author's declared
    /// source, scope, units, and quality verbatim.
    pub fn from_records(records: Vec<KnowledgeRecord>) -> Self {
        let mut store = Self::default();
        for record in records {
            store.insert(record);
        }
        store
    }

    /// `trusted_count` is the number of hand-authored laws before a scraped
    /// cache is appended. The latter are always marked Candidate.
    pub fn from_laws(
        laws: &[PhysicsLaw],
        trusted_count: usize,
        candidate_source: &str,
        default_domain: &str,
    ) -> Self {
        let mut store = Self::default();
        for (index, law) in laws.iter().enumerate() {
            let quality = if index < trusted_count {
                KnowledgeQuality::Trusted
            } else {
                KnowledgeQuality::Candidate
            };
            store.insert(KnowledgeRecord {
                id: law.name.clone(),
                statement: law.description.clone(),
                formula: Some(law.formula.clone()),
                source: if quality == KnowledgeQuality::Trusted {
                    "hand-curated deterministic law set".to_string()
                } else {
                    candidate_source.to_string()
                },
                domain: law
                    .tags
                    .first()
                    .cloned()
                    .unwrap_or_else(|| default_domain.to_string()),
                variables: law.variables.clone(),
                units: HashMap::new(),
                quality,
                entailment_examples: Vec::new(),
                assumptions: if quality == KnowledgeQuality::Candidate {
                    vec!["source conditions have not been independently reviewed".to_string()]
                } else {
                    vec!["input quantities satisfy the deterministic solver model".to_string()]
                },
            });
        }
        store
    }

    pub fn insert(&mut self, record: KnowledgeRecord) {
        let index = self.records.len();
        self.by_id.insert(record.id.clone(), index);
        self.records.push(record);
    }
    pub fn record(&self, id: &str) -> Option<&KnowledgeRecord> {
        self.by_id
            .get(id)
            .and_then(|index| self.records.get(*index))
    }

    /// Retrieve a source passage that is independently answerable under the
    /// store's quality and applicability rules.  A lexical/semantic hit is
    /// only candidate generation; Candidate records never cross this gate.
    pub fn retrieve_entailed_passage(
        &self,
        question: &str,
        expected_domain: &str,
    ) -> Option<&KnowledgeRecord> {
        self.retrieve_candidates(question, 8)
            .into_iter()
            .find(|record| {
                record.quality != KnowledgeQuality::Candidate
                    && !record.source.trim().is_empty()
                    && !record.assumptions.is_empty()
                    && question_mentions_record_anchor(question, record)
                    && question_entails_record(question, record)
                    && assumptions_apply_to_question(question, record)
                    && (expected_domain.is_empty()
                        || domain_compatible(&record.domain, expected_domain))
                    && self.verify_derivation(question, &[record.id.clone()], expected_domain)
                        == EntailmentVerdict::Entailed
            })
    }

    /// Semantic retrieval is candidate generation only.  Its transparent
    /// synonym expansion improves recall without granting evidential status.
    pub fn retrieve_candidates(&self, question: &str, limit: usize) -> Vec<&KnowledgeRecord> {
        let query = expanded_terms(question);
        let mut ranked: Vec<(&KnowledgeRecord, usize)> = self
            .records
            .iter()
            .filter_map(|record| {
                // A domain label (for example `life_science`) is a routing hint,
                // not evidence that this particular passage answers the question.
                // Ranking on it made every chemistry/biology question look like a
                // match for an unrelated curated water fact.  Candidate retrieval
                // must share substantive statement/formula terms instead.
                let text = format!(
                    "{} {} {}",
                    record.id,
                    record.statement,
                    record.formula.as_deref().unwrap_or("")
                );
                let overlap = query.intersection(&expanded_terms(&text)).count();
                (overlap >= 2).then_some((record, overlap))
            })
            .collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.id.cmp(&b.0.id)));
        ranked
            .into_iter()
            .take(limit)
            .map(|(record, _)| record)
            .collect()
    }

    /// Check source quality, declared conditions and domain before an answer.
    pub fn verify_derivation(
        &self,
        question: &str,
        law_ids: &[String],
        expected_domain: &str,
    ) -> EntailmentVerdict {
        if law_ids.is_empty() {
            return EntailmentVerdict::MissingSource;
        }
        let question_domain = detected_domain(question);
        for id in law_ids {
            let Some(record) = self.record(id) else {
                return EntailmentVerdict::MissingSource;
            };
            if record.quality == KnowledgeQuality::Candidate {
                return EntailmentVerdict::CandidateOnly;
            }
            if (!expected_domain.is_empty() && !domain_compatible(&record.domain, expected_domain))
                || question_domain
                    .as_deref()
                    .is_some_and(|domain| !domain_compatible(&record.domain, domain))
            {
                return EntailmentVerdict::DomainMismatch;
            }
            if record.source.trim().is_empty() || record.assumptions.is_empty() {
                return EntailmentVerdict::ConditionsNotEstablished;
            }
        }
        EntailmentVerdict::Entailed
    }
}

fn domain_compatible(record_domain: &str, requested: &str) -> bool {
    let record = record_domain.to_lowercase();
    let requested = requested.to_lowercase();
    record == requested
        || record.contains(&requested)
        || requested.contains(&record)
        || (requested == "physics"
            && [
                "mechanics",
                "optics",
                "thermodynamics",
                "electromagnetism",
                "waves",
                "fluids",
                "astronomy",
            ]
            .iter()
            .any(|part| record.contains(part)))
        || (requested == "mathematics"
            && [
                "algebra", "calculus", "geometry", "analysis", "topology", "number",
            ]
            .iter()
            .any(|part| record.contains(part)))
}
fn detected_domain(question: &str) -> Option<String> {
    let q = question.to_lowercase();
    if [
        "force", "mass", "velocity", "orbit", "energy", "wave", "mirror", "pressure",
    ]
    .iter()
    .any(|word| q.contains(word))
    {
        Some("physics".to_string())
    } else if [
        "derivative",
        "integral",
        "prime",
        "equation",
        "algebra",
        "triangle",
    ]
    .iter()
    .any(|word| q.contains(word))
    {
        Some("mathematics".to_string())
    } else {
        None
    }
}

/// A shared generic phrase (for example, "molecular formula") identifies a
/// topic, not the fact being asserted.  Require at least one record-specific
/// token such as `water`, `h2o`, or `dna` before a curated passage can answer.
/// This is intentionally conservative: an unanchored paraphrase abstains
/// until it has a real entailment model behind it.
fn question_mentions_record_anchor(question: &str, record: &KnowledgeRecord) -> bool {
    const GENERIC_EVIDENCE_TERMS: &[&str] = &[
        "cellular",
        "biology",
        "chemistry",
        "computer",
        "complexity",
        "constant",
        "formula",
        "genetic",
        "information",
        "molecular",
        "medicine",
        "number",
        "organism",
        "organisms",
        "rate",
        "store",
        "time",
        "asymptotic",
        "growth",
    ];
    let query = expanded_terms(question);
    // The prose statement is evidence after retrieval, not an anchor.  If it
    // were used here, a generic wording such as "what constant is used" could
    // anchor itself on the answer sentence's word "constant".  IDs, declared
    // variables and a formula name are the inspectable entity anchors.
    let mut record_terms = expanded_terms(&format!(
        "{} {} {}",
        record.id,
        record.variables.join(" "),
        record.formula.as_deref().unwrap_or(""),
    ));
    record_terms.retain(|term| !GENERIC_EVIDENCE_TERMS.contains(&term.as_str()));
    !record_terms.is_empty() && !query.is_disjoint(&record_terms)
}

/// Conservative entailment gate for the deliberately small curated pack. A
/// source record is allowed to answer only an explicitly audited,
/// record-specific question form (case/punctuation differences are harmless).
/// Retrieval similarity, shared variables, and partial lexical overlap are
/// never entailment: those were sufficient to turn a B-cell sequencing prompt
/// into a hemoglobin answer. Until a real NLI verifier is available, new
/// paraphrases must be reviewed and added to `entailment_examples`.
fn question_entails_record(question: &str, record: &KnowledgeRecord) -> bool {
    let canonical = |text: &str| {
        text.to_ascii_lowercase()
            .chars()
            .filter(|ch| ch.is_alphanumeric())
            .collect::<String>()
    };
    let question = canonical(question);
    !question.is_empty()
        && record
            .entailment_examples
            .iter()
            .any(|example| canonical(example) == question)
}

fn assumptions_apply_to_question(question: &str, record: &KnowledgeRecord) -> bool {
    let question = question.to_ascii_lowercase();
    let assumptions = record.assumptions.join(" ").to_ascii_lowercase();
    // A source explicitly scoped to ordinary biology cannot establish claims
    // about hypothetical non-standard or quantum life.  More detailed
    // assumption matching belongs in a future domain verifier; this guard
    // prevents the unsafe direction today.
    !(assumptions.contains("standard cellular biology")
        && [
            "non-standard",
            "quantum",
            "hypothetical",
            "8 distinct nucleotide",
        ]
        .iter()
        .any(|marker| question.contains(marker)))
}
fn expanded_terms(text: &str) -> HashSet<String> {
    // Function words are ubiquitous in both questions and passages.  Counting
    // them as overlap made an unrelated record look entailed merely because
    // both strings contained words such as "the" and "for".
    const STOPWORDS: &[&str] = &[
        "the", "and", "are", "but", "for", "from", "has", "have", "how", "into", "not", "that",
        "this", "was", "what", "when", "where", "which", "with", "would", "you",
    ];
    let mut terms: HashSet<String> = text
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| word.len() > 2 && !STOPWORDS.contains(word))
        .map(str::to_string)
        .collect();
    // Small deterministic normalization improves retrieval of source passages
    // without claiming semantic equivalence (raised→raise, equations→equation).
    let normalized: Vec<String> = terms
        .iter()
        .filter_map(|word| {
            if word.ends_with("ies") && word.len() > 4 {
                Some(format!("{}y", &word[..word.len() - 3]))
            } else if word.ends_with("sed") && word.len() > 4 {
                Some(word[..word.len() - 1].to_string())
            } else if word.ends_with("ed") && word.len() > 4 {
                Some(word.trim_end_matches("ed").to_string())
            } else if word.ends_with('s') && word.len() > 4 {
                Some(word[..word.len() - 1].to_string())
            } else {
                None
            }
        })
        .collect();
    terms.extend(normalized);
    for (left, right) in [
        ("speed", "velocity"),
        ("weight", "force"),
        ("work", "energy"),
        ("differentiate", "derivative"),
    ] {
        if terms.contains(left) {
            terms.insert(right.to_string());
        }
        if terms.contains(right) {
            terms.insert(left.to_string());
        }
    }
    terms
}

#[cfg(test)]
mod tests {
    use super::*;
    fn law(name: &str) -> PhysicsLaw {
        PhysicsLaw {
            name: name.to_string(),
            description: "force is mass times acceleration".to_string(),
            formula: "F=m*a".to_string(),
            tags: vec!["physics".to_string()],
            variables: vec!["F".to_string(), "m".to_string(), "a".to_string()],
            target_var: "F".to_string(),
        }
    }
    #[test]
    fn candidates_are_retrievable_but_not_entailing() {
        let store = CuratedKnowledgeStore::from_laws(
            &[law("newton"), law("scraped_force")],
            1,
            "Wikipedia",
            "physics",
        );
        assert_eq!(
            store
                .retrieve_candidates("What force acts on a mass?", 5)
                .len(),
            2
        );
        assert_eq!(
            store.verify_derivation(
                "Find force on mass",
                &["scraped_force".to_string()],
                "physics"
            ),
            EntailmentVerdict::CandidateOnly
        );
    }
    #[test]
    fn trusted_source_with_matching_domain_entails() {
        let store = CuratedKnowledgeStore::from_laws(&[law("newton")], 1, "Wikipedia", "physics");
        assert_eq!(
            store.verify_derivation("Find force on mass", &["newton".to_string()], "physics"),
            EntailmentVerdict::Entailed
        );
    }

    #[test]
    fn declared_entailment_examples_are_anchored_and_source_backed() {
        let record = KnowledgeRecord {
            id: "cs_binary_search_sorted".to_string(),
            statement: "Binary search runs in logarithmic time on a sorted sequence.".to_string(),
            formula: Some("T(n) = O(log n)".to_string()),
            source: "CLRS, Introduction to Algorithms, Chapter 2".to_string(),
            domain: "computer_science".to_string(),
            variables: vec!["binary_search".to_string(), "sorted_sequence".to_string()],
            assumptions: vec!["The input sequence is sorted under the searched key.".to_string()],
            units: HashMap::new(),
            quality: KnowledgeQuality::Curated,
            entailment_examples: vec![
                "What is the time complexity of binary search on a sorted sequence?".to_string(),
            ],
        };
        let store = CuratedKnowledgeStore::from_records(vec![record.clone()]);
        for example in &record.entailment_examples {
            assert_eq!(
                store
                    .retrieve_entailed_passage(example, "")
                    .map(|found| found.id.as_str()),
                Some(record.id.as_str())
            );
        }
    }

    #[test]
    fn curated_record_rejects_topic_overlap_without_its_entailment_shape() {
        let record = KnowledgeRecord {
            id: "biology_cell_basic_unit".to_string(),
            statement: "The cell is the basic unit of structure and function in living organisms."
                .to_string(),
            formula: None,
            source: "reviewed biology source".to_string(),
            domain: "biology".to_string(),
            variables: vec!["cell".to_string(), "organism".to_string()],
            assumptions: vec!["ordinary cellular life".to_string()],
            units: HashMap::new(),
            quality: KnowledgeQuality::Curated,
            entailment_examples: vec![
                "In standard cell theory, what is the basic unit of living organisms?".to_string(),
            ],
        };
        let store = CuratedKnowledgeStore::from_records(vec![record]);
        assert!(store
            .retrieve_entailed_passage(
                "Single-cell sequencing recovered two light chains from a B cell.",
                ""
            )
            .is_none());
    }
}
