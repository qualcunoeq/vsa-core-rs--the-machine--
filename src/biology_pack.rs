//! Source-derived bounded molecular-biology representations.
//!
//! This first biology capability is deliberately narrow: exact DNA alphabet
//! validation, aligned complements, reverse complements, and base composition.
//! It does not infer strand orientation, transcribe RNA, translate codons, or
//! make claims about genes, mutations, or phenotype.

use super::SourceCitation;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[path = "biology_frontend.rs"]
pub mod biology_frontend;

#[path = "biology_probability_bridge.rs"]
pub mod biology_probability_bridge;

const DOMAIN: &str = "source_derived_bounded_dna";
const MAX_SEQUENCE: usize = 256;

fn source() -> SourceCitation {
    SourceCitation {
        source_id: "openstax-biology-2e:dna-complementary-pairing".into(),
        title: "Biology 2e".into(),
        section: "3.5 Nucleic Acids; 14.2 DNA Structure and Sequencing".into(),
        url: "https://openstax.org/books/biology-2e/pages/14-2-dna-structure-and-sequencing".into(),
        license: "CC BY 4.0; OpenStax attribution required".into(),
        retrieved_utc: "2026-08-16".into(),
        evidence_span: "DNA uses the bases A, T, C, and G; A pairs with T and G pairs with C".into(),
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BiologyOperation {
    ValidateDna,
    Complement,
    ReverseComplement,
    BaseComposition,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BiologyStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidDomain,
    Inconsistent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BiologyArtifact {
    DnaSequence {
        sequence: String,
        orientation: String,
    },
    PairedComplement {
        source: String,
        complement: String,
        source_orientation: String,
        complement_orientation: String,
    },
    BaseComposition {
        length: u32,
        counts: BTreeMap<String, u32>,
        gc_count: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BiologyRequest {
    pub operation: BiologyOperation,
    pub sequence: Option<String>,
    pub orientation: Option<String>,
    pub domain: String,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BiologyResult {
    pub status: BiologyStatus,
    pub artifact: Option<BiologyArtifact>,
    pub operation: BiologyOperation,
    pub assumptions: Vec<String>,
    pub source: Option<SourceCitation>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("biology serializes"))
    )
}

fn payload(result: &BiologyResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.artifact,
        result.operation,
        &result.assumptions,
        &result.source,
        &result.reasons,
        &result.provenance,
    )
}

fn assumptions() -> Vec<String> {
    vec![
        "DNA alphabet is exactly A, T, C, and G".into(),
        "complementary pairing is A-T and G-C".into(),
        "strand orientation is explicit for complement operations".into(),
        "RNA, codons, translation, mutation, and phenotype semantics are outside scope".into(),
    ]
}

fn result(
    request: &BiologyRequest,
    status: BiologyStatus,
    artifact: Option<BiologyArtifact>,
    source: Option<SourceCitation>,
    reasons: Vec<String>,
) -> BiologyResult {
    let mut output = BiologyResult {
        status,
        artifact,
        operation: request.operation,
        assumptions: assumptions(),
        source,
        reasons,
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&output));
    output.replay_hash = replay_hash;
    output
}

fn normalize_sequence(sequence: &str) -> Result<String, String> {
    let normalized = sequence.trim().to_ascii_uppercase();
    if normalized.is_empty() {
        return Err("DNA sequence is empty".into());
    }
    if normalized.len() > MAX_SEQUENCE {
        return Err("DNA sequence exceeds the bounded length".into());
    }
    if normalized.chars().any(|base| !matches!(base, 'A' | 'T' | 'C' | 'G')) {
        return Err("sequence contains a non-DNA base or RNA symbol".into());
    }
    Ok(normalized)
}

fn paired_base(base: char) -> char {
    match base {
        'A' => 'T',
        'T' => 'A',
        'C' => 'G',
        'G' => 'C',
        _ => unreachable!("validated DNA base"),
    }
}

fn aligned_complement(sequence: &str) -> String {
    sequence.chars().map(paired_base).collect()
}

fn reverse_complement(sequence: &str) -> String {
    sequence.chars().rev().map(paired_base).collect()
}

fn composition(sequence: &str) -> BiologyArtifact {
    let mut counts = BTreeMap::new();
    for base in ['A', 'C', 'G', 'T'] {
        counts.insert(base.to_string(), sequence.chars().filter(|value| *value == base).count() as u32);
    }
    BiologyArtifact::BaseComposition {
        length: sequence.len() as u32,
        gc_count: counts["G"] + counts["C"],
        counts,
    }
}

pub fn evaluate_biology(request: &BiologyRequest) -> BiologyResult {
    let cited = source();
    if request.domain != DOMAIN {
        return result(
            request,
            BiologyStatus::InvalidDomain,
            None,
            None,
            vec!["domain is outside bounded DNA biology".into()],
        );
    }
    if let Some(ambiguity) = &request.ambiguity {
        return result(
            request,
            BiologyStatus::Ambiguous,
            None,
            None,
            vec![ambiguity.clone()],
        );
    }
    let Some(sequence) = request.sequence.as_deref() else {
        return result(
            request,
            BiologyStatus::Missing,
            None,
            Some(cited),
            vec!["one DNA sequence is required".into()],
        );
    };
    let sequence = match normalize_sequence(sequence) {
        Ok(sequence) => sequence,
        Err(reason) if reason.contains("RNA") || reason.contains("non-DNA") => {
            return result(
                request,
                BiologyStatus::Unsupported,
                None,
                Some(cited),
                vec![reason],
            );
        }
        Err(reason) => {
            return result(
                request,
                BiologyStatus::Inconsistent,
                None,
                Some(cited),
                vec![reason],
            );
        }
    };
    match request.operation {
        BiologyOperation::ValidateDna => result(
            request,
            BiologyStatus::Complete,
            Some(BiologyArtifact::DnaSequence {
                sequence,
                orientation: request
                    .orientation
                    .clone()
                    .unwrap_or_else(|| "unspecified".into()),
            }),
            Some(cited),
            Vec::new(),
        ),
        BiologyOperation::BaseComposition => result(
            request,
            BiologyStatus::Complete,
            Some(composition(&sequence)),
            Some(cited),
            Vec::new(),
        ),
        BiologyOperation::Complement => {
            if request.orientation.as_deref() != Some("5_to_3") {
                return result(
                    request,
                    BiologyStatus::Ambiguous,
                    None,
                    Some(cited),
                    vec!["aligned complement requires an explicit 5_to_3 source orientation".into()],
                );
            }
            result(
                request,
                BiologyStatus::Complete,
                Some(BiologyArtifact::PairedComplement {
                    source: sequence.clone(),
                    complement: aligned_complement(&sequence),
                    source_orientation: "5_to_3".into(),
                    complement_orientation: "3_to_5".into(),
                }),
                Some(cited),
                Vec::new(),
            )
        }
        BiologyOperation::ReverseComplement => {
            if request.orientation.as_deref() != Some("5_to_3") {
                return result(
                    request,
                    BiologyStatus::Ambiguous,
                    None,
                    Some(cited),
                    vec!["reverse complement requires an explicit 5_to_3 source orientation".into()],
                );
            }
            let complement = reverse_complement(&sequence);
            result(
                request,
                BiologyStatus::Complete,
                Some(BiologyArtifact::PairedComplement {
                    source: sequence,
                    complement,
                    source_orientation: "5_to_3".into(),
                    complement_orientation: "5_to_3".into(),
                }),
                Some(cited),
                Vec::new(),
            )
        }
    }
}

impl BiologyResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != BiologyStatus::Complete || self.artifact.is_some())
            && (self.status != BiologyStatus::Complete || self.source.is_some())
    }

    pub fn authorized(&self) -> bool {
        self.status == BiologyStatus::Complete && self.replay_verified()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(operation: BiologyOperation) -> BiologyRequest {
        BiologyRequest {
            operation,
            sequence: Some("AATTGGCC".into()),
            orientation: Some("5_to_3".into()),
            domain: DOMAIN.into(),
            ambiguity: None,
            provenance: vec!["biology-test".into()],
        }
    }

    #[test]
    fn complement_and_composition_are_replayable() {
        let complement = evaluate_biology(&request(BiologyOperation::ReverseComplement));
        assert!(complement.authorized());
        assert!(matches!(
            complement.artifact,
            Some(BiologyArtifact::PairedComplement { complement, .. }) if complement == "GGCC AATT".replace(' ', "")
        ));
        let composition = evaluate_biology(&request(BiologyOperation::BaseComposition));
        assert!(composition.authorized());
    }

    #[test]
    fn rna_and_missing_orientation_fail_closed() {
        let mut rna = request(BiologyOperation::ValidateDna);
        rna.sequence = Some("AUGC".into());
        assert_eq!(evaluate_biology(&rna).status, BiologyStatus::Unsupported);
        let mut ambiguous = request(BiologyOperation::ReverseComplement);
        ambiguous.orientation = None;
        assert_eq!(evaluate_biology(&ambiguous).status, BiologyStatus::Ambiguous);
    }
}
