//! Phase 23 blinded audit for technical-notation contamination.
//!
//! This pass deliberately does not read expected answers, terminal
//! classifications, or answer keys.  It only uses the question text, the
//! broad source category, and deterministic syntax markers from Phase 22.
//! The output is a review queue, not parser authorization or a knowledge
//! decision.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;

#[derive(Debug, Deserialize)]
struct InputReport {
    source_trace_sha256: String,
    records: Vec<InputRecord>,
}

#[derive(Debug, Deserialize)]
struct InputRecord {
    id: Option<String>,
    category: String,
    mechanism: String,
    question: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Domain {
    Mathematics,
    Physics,
    Chemistry,
    BiologyMedicine,
    ComputerScience,
    StatisticsInformation,
    OtherTechnical,
    MixedOrUnknown,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum NotationFamily {
    EquationsAndExpressions,
    SetLogicAndQuantifiers,
    LinearAlgebraAndMatrices,
    ProbabilityStatisticsAndInformation,
    DifferentialOrDynamicalSystems,
    GeometryAndTopology,
    ChemicalFormulaOrStructure,
    BiologicalOrMedicalNomenclature,
    FormalLanguageOrCode,
    GameBoardOrDiagram,
    SpecializedNamedNotation,
    MixedOrUnknown,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum InterpretationStatus {
    LikelyUniqueTypedInterpretation,
    LocallyDefinedButNeedsReview,
    ExternalConventionRequired,
    AmbiguousOrMalformed,
    VisualStructureRequired,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum DownstreamOutlook {
    LikelyNormalizationOnly,
    LikelyKnowledgeOrReasoningGapRemains,
    VisualOrExternalDependency,
    NeedsManualReview,
}

#[derive(Debug, Serialize)]
struct NotationRecord {
    id: Option<String>,
    category: String,
    source_mechanism: String,
    domain: Domain,
    notation_family: NotationFamily,
    locally_defined_symbols: bool,
    external_convention_required: bool,
    interpretation: InterpretationStatus,
    downstream_outlook: DownstreamOutlook,
    confidence: String,
    reasons: Vec<String>,
    question: String,
}

#[derive(Debug, Serialize)]
struct Report {
    input_audit_sha256: String,
    source_trace_sha256: String,
    scanned_rows: usize,
    notation_rows: usize,
    source_mechanisms: BTreeMap<String, usize>,
    domains: BTreeMap<Domain, usize>,
    notation_families: BTreeMap<NotationFamily, usize>,
    interpretations: BTreeMap<InterpretationStatus, usize>,
    downstream_outlooks: BTreeMap<DownstreamOutlook, usize>,
    samples: BTreeMap<NotationFamily, Vec<String>>,
    records: Vec<NotationRecord>,
    method: String,
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn has_any(text: &str, terms: &[&str]) -> bool {
    terms.iter().any(|term| text.contains(term))
}

fn balanced(text: &str, open: char, close: char) -> bool {
    let mut depth = 0i32;
    for ch in text.chars() {
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth < 0 {
                return false;
            }
        }
    }
    depth == 0
}

fn domain(category: &str, text: &str) -> Domain {
    let category = category.to_ascii_lowercase();
    if category.contains("physics")
        || has_any(
            text,
            &[
                "gamma",
                "hamiltonian",
                "lagrangian",
                "boson",
                "fermion",
                "velocity",
                "mass",
            ],
        )
    {
        Domain::Physics
    } else if category.contains("chem")
        || has_any(
            text,
            &["molecule", "reaction", "chemical", "isomer", "oxidation"],
        )
    {
        Domain::Chemistry
    } else if category.contains("biology")
        || category.contains("medicine")
        || has_any(
            text,
            &["genome", "protein", "cell", "species", "gene", "syndrome"],
        )
    {
        Domain::BiologyMedicine
    } else if category.contains("computer")
        || category.contains("ai")
        || has_any(
            text,
            &[
                "automaton",
                "regular expression",
                "algorithm",
                "vc dimension",
                "classifier",
                "program",
            ],
        )
    {
        Domain::ComputerScience
    } else if has_any(
        text,
        &[
            "mutual information",
            "entropy",
            "random variable",
            "probability",
            "markov chain",
            "expectation",
        ],
    ) {
        Domain::StatisticsInformation
    } else if category.contains("math") {
        Domain::Mathematics
    } else if category.contains("other") {
        Domain::OtherTechnical
    } else {
        Domain::MixedOrUnknown
    }
}

fn notation_family(text: &str, domain: Domain) -> NotationFamily {
    if has_any(
        text,
        &[
            "\\begin{bmatrix}",
            "matrix",
            "eigenvalue",
            "rank-",
            "determinant",
            "vector",
            "\u{03c3}_",
            "\\sigma_",
        ],
    ) {
        return NotationFamily::LinearAlgebraAndMatrices;
    }
    if has_any(
        text,
        &[
            "mutual information",
            "entropy",
            "random variable",
            "probability",
            "expectation",
            "markov chain",
            "covariance",
        ],
    ) {
        return NotationFamily::ProbabilityStatisticsAndInformation;
    }
    if has_any(
        text,
        &[
            "\\frac{d",
            "differential equation",
            "boundary-value",
            "stability",
            "derivative",
            "integral",
            "t \\to",
            "t->",
        ],
    ) {
        return NotationFamily::DifferentialOrDynamicalSystems;
    }
    if has_any(
        text,
        &[
            "topological",
            "homotopy",
            "manifold",
            "hypersurface",
            "knot",
            "genus",
            "\u{03c0}_",
            "\\pi_",
        ],
    ) {
        return NotationFamily::GeometryAndTopology;
    }
    if has_any(
        text,
        &[
            "chemical",
            "molecule",
            "reaction",
            "molar",
            "oxidation",
            "protein",
            "dna",
            "rna",
        ],
    ) {
        return if matches!(domain, Domain::Chemistry) {
            NotationFamily::ChemicalFormulaOrStructure
        } else {
            NotationFamily::BiologicalOrMedicalNomenclature
        };
    }
    if has_any(
        text,
        &[
            "regular expression",
            "automaton",
            "language l",
            "uci",
            "fen",
            "program",
            "code",
            "classifier",
        ],
    ) {
        return if has_any(
            text,
            &["sudoku", "reversi", "chess", "board", "plot", "picture"],
        ) {
            NotationFamily::GameBoardOrDiagram
        } else {
            NotationFamily::FormalLanguageOrCode
        };
    }
    if has_any(
        text,
        &[
            "subset",
            "powerset",
            "set of",
            "for all",
            "there exists",
            "iff",
            "poset",
            "\u{2208}",
            "\u{2286}",
        ],
    ) {
        return NotationFamily::SetLogicAndQuantifiers;
    }
    if has_any(
        text,
        &[
            "\\frac",
            "=",
            "<",
            ">",
            "inequality",
            "equation",
            "formula",
            "calculate",
            "solve",
        ],
    ) {
        return NotationFamily::EquationsAndExpressions;
    }
    if matches!(
        domain,
        Domain::Physics | Domain::Chemistry | Domain::BiologyMedicine
    ) {
        NotationFamily::SpecializedNamedNotation
    } else {
        NotationFamily::MixedOrUnknown
    }
}

fn audit_record(row: InputRecord) -> NotationRecord {
    let text = row.question.to_ascii_lowercase();
    let domain = domain(&row.category, &text);
    let family = notation_family(&text, domain);
    let locally_defined = has_any(
        &text,
        &[
            "let ",
            "define",
            "denote",
            "where ",
            "suppose",
            "given",
            "consider",
            "we have",
            "is defined",
        ],
    );
    let external = has_any(
        &text,
        &[
            "standard",
            "well-known",
            "usual notation",
            "in physics",
            "in chemistry",
            "named",
            "theorem",
            "law of",
            "gamma matrices",
            "hurwitz",
            "cartan",
            "dehn twist",
            "wasserstein",
            "algebraic topology",
        ],
    );
    let visual = has_any(
        &text,
        &[
            "picture",
            "image",
            "plot",
            "diagram",
            "figure",
            "appended",
            "shown below",
            "board",
            "grid",
        ],
    );
    let malformed =
        !balanced(&text, '(', ')') || !balanced(&text, '{', '}') || !balanced(&text, '[', ']');
    let unresolved = has_any(
        &text,
        &[
            "some ",
            "unknown ",
            "not defined",
            "without defining",
            "what does .* mean",
            "where .* is not specified",
        ],
    );
    let interpretation = if visual {
        InterpretationStatus::VisualStructureRequired
    } else if malformed || unresolved {
        InterpretationStatus::AmbiguousOrMalformed
    } else if external && !locally_defined {
        InterpretationStatus::ExternalConventionRequired
    } else if locally_defined {
        InterpretationStatus::LocallyDefinedButNeedsReview
    } else {
        InterpretationStatus::LikelyUniqueTypedInterpretation
    };
    let outlook = match interpretation {
        InterpretationStatus::LikelyUniqueTypedInterpretation
        | InterpretationStatus::LocallyDefinedButNeedsReview => {
            if external
                || matches!(
                    family,
                    NotationFamily::SpecializedNamedNotation | NotationFamily::GameBoardOrDiagram
                )
            {
                DownstreamOutlook::LikelyKnowledgeOrReasoningGapRemains
            } else {
                DownstreamOutlook::LikelyNormalizationOnly
            }
        }
        InterpretationStatus::ExternalConventionRequired => {
            DownstreamOutlook::LikelyKnowledgeOrReasoningGapRemains
        }
        InterpretationStatus::VisualStructureRequired => {
            DownstreamOutlook::VisualOrExternalDependency
        }
        InterpretationStatus::AmbiguousOrMalformed => DownstreamOutlook::NeedsManualReview,
    };
    let mut reasons = Vec::new();
    reasons.push(format!(
        "domain inferred from category/text as {:?}",
        domain
    ));
    reasons.push(format!("notation family inferred as {:?}", family));
    if locally_defined {
        reasons.push("local definition cue present".into());
    }
    if external {
        reasons.push("external convention cue present".into());
    }
    if visual {
        reasons.push("visual/layout dependency cue present".into());
    }
    if malformed {
        reasons.push("delimiter balance requires review".into());
    }
    if unresolved {
        reasons.push("unresolved-binding cue present".into());
    }
    NotationRecord {
        id: row.id,
        category: row.category,
        source_mechanism: row.mechanism,
        domain,
        notation_family: family,
        locally_defined_symbols: locally_defined,
        external_convention_required: external,
        interpretation,
        downstream_outlook: outlook,
        confidence: if matches!(
            interpretation,
            InterpretationStatus::LikelyUniqueTypedInterpretation
        ) {
            "medium"
        } else {
            "low"
        }
        .into(),
        reasons,
        question: row.question,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/hle_normalization_audit_2147e9e.json".into());
    let output = env::args()
        .nth(2)
        .unwrap_or_else(|| "/tmp/hle_notation_audit_2147e9e.json".into());
    let bytes = fs::read(&input)?;
    let input_hash = sha256(&bytes);
    let report: InputReport = serde_json::from_slice(&bytes)?;
    let scanned_rows = report.records.len();
    let source_trace_sha256 = report.source_trace_sha256.clone();
    let mut domains = BTreeMap::new();
    let mut source_mechanisms = BTreeMap::new();
    let mut families = BTreeMap::new();
    let mut interpretations = BTreeMap::new();
    let mut outlooks = BTreeMap::new();
    let mut samples: BTreeMap<NotationFamily, Vec<String>> = BTreeMap::new();
    let mut records = Vec::new();
    for row in report.records.into_iter().filter(|row| {
        matches!(
            row.mechanism.as_str(),
            "specialist_notation" | "embedded_formula"
        )
    }) {
        let record = audit_record(row);
        *source_mechanisms
            .entry(record.source_mechanism.clone())
            .or_insert(0) += 1;
        *domains.entry(record.domain).or_insert(0) += 1;
        *families.entry(record.notation_family).or_insert(0) += 1;
        *interpretations.entry(record.interpretation).or_insert(0) += 1;
        *outlooks.entry(record.downstream_outlook).or_insert(0) += 1;
        let sample = samples.entry(record.notation_family).or_default();
        if sample.len() < 5 {
            sample.push(format!(
                "{}: {}",
                record.id.as_deref().unwrap_or("no-id"),
                record.question.replace('\n', " ")
            ));
        }
        records.push(record);
    }
    let output_report = Report {
        input_audit_sha256: input_hash,
        source_trace_sha256,
        scanned_rows,
        notation_rows: records.len(),
        source_mechanisms,
        domains,
        notation_families: families,
        interpretations,
        downstream_outlooks: outlooks,
        samples,
        records,
        method: "blinded deterministic notation-family audit using only category, mechanism marker, and question syntax; no answers, terminal labels, or parser mutation".into(),
    };
    fs::write(&output, serde_json::to_vec_pretty(&output_report)?)?;
    println!("{}", serde_json::to_string_pretty(&output_report)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(question: &str, category: &str, mechanism: &str) -> InputRecord {
        InputRecord {
            id: Some("test".into()),
            category: category.into(),
            mechanism: mechanism.into(),
            question: question.into(),
        }
    }

    #[test]
    fn audit_is_blind_and_deterministic() {
        let record = audit_record(row(
            "Let A = \\begin{bmatrix}1&0\\end{bmatrix}. Find rank(A).",
            "Math",
            "specialist_notation",
        ));
        assert_eq!(record.domain, Domain::Mathematics);
        assert_eq!(
            record.notation_family,
            NotationFamily::LinearAlgebraAndMatrices
        );
        assert!(record.locally_defined_symbols);
        assert_eq!(
            audit_record(row("5 kg + 3 seconds", "Physics", "embedded_formula")).interpretation,
            InterpretationStatus::LikelyUniqueTypedInterpretation
        );
    }

    #[test]
    fn malformed_and_visual_cases_do_not_claim_unique_parse() {
        assert_eq!(
            audit_record(row(
                "Read the appended picture [",
                "Other",
                "specialist_notation"
            ))
            .interpretation,
            InterpretationStatus::VisualStructureRequired
        );
        assert_eq!(
            audit_record(row("Compute (x + 1", "Math", "embedded_formula")).interpretation,
            InterpretationStatus::AmbiguousOrMalformed
        );
    }
}
