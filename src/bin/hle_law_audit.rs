//! Phase 30 shadow audit of the HLE equations/scientific-law pool.
//!
//! This extracts reusable law and bridge evidence without retrieving sources,
//! authorizing answers, or changing the production registry.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;

const DATASET: &str = "data/hle.jsonl";

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum LawFamily {
    TransformerCost,
    CausalTreatment,
    ChemicalKinetics,
    ReactionStoichiometry,
    Thermodynamics,
    Electromagnetism,
    Mechanics,
    QuantumPhysics,
    StatisticalEstimator,
    PopulationGenetics,
    InformationTheory,
    ProbabilityFormula,
    AlgebraicIdentity,
    OtherSpecialist,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum AuditOutcome {
    RetrievalReadyEquation,
    InQuestionEquation,
    DerivationAfterRetrieval,
    MissingPrerequisite,
    ConventionOrDefinition,
    SpecialistSingleton,
}

#[derive(Debug, Deserialize)]
struct KnowledgeAudit {
    input_trace_sha256: String,
    records: Vec<KnowledgeRecord>,
}

#[derive(Debug, Deserialize)]
struct KnowledgeRecord {
    id: String,
    category: String,
    gap: String,
    question: String,
}

#[derive(Debug, Serialize)]
struct LawCase {
    id: String,
    category: String,
    family: LawFamily,
    law_cues: Vec<String>,
    variables: Vec<String>,
    units: Vec<String>,
    assumptions: Vec<String>,
    requested_output: String,
    law_stated_in_question: bool,
    retrieval_sufficient: bool,
    nearest_existing_capability: String,
    bridge_primitives: Vec<String>,
    outcome: AuditOutcome,
}

#[derive(Debug, Serialize)]
struct FamilySummary {
    family: LawFamily,
    cases: usize,
    case_ids: Vec<String>,
    output_types: BTreeMap<String, usize>,
    repeated_law_cues: BTreeMap<String, usize>,
    retrieval_sufficient_cases: usize,
    independent_corpus_required: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    knowledge_audit_sha256: String,
    input_trace_sha256: String,
    dataset_sha256: String,
    law_cases: usize,
    outcomes: BTreeMap<AuditOutcome, usize>,
    family_counts: BTreeMap<LawFamily, usize>,
    bridge_primitive_counts: BTreeMap<String, usize>,
    repeated_families: usize,
    cases: Vec<LawCase>,
    families: Vec<FamilySummary>,
    method: String,
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn has_any(text: &str, terms: &[&str]) -> bool {
    terms.iter().any(|term| text.contains(term))
}

fn family(category: &str, question: &str) -> LawFamily {
    let text = question.to_ascii_lowercase();
    let category = category.to_ascii_lowercase();
    if has_any(&text, &["transformer", "attention heads", "context length"]) {
        LawFamily::TransformerCost
    } else if has_any(
        &text,
        &["treatment effect", "binary", "controls x", "causal"],
    ) {
        LawFamily::CausalTreatment
    } else if has_any(
        &text,
        &["arrhenius", "reaction rate", "kinetic", "activation energy"],
    ) {
        LawFamily::ChemicalKinetics
    } else if has_any(
        &text,
        &["stoichiometr", "moles", "reagent", "reaction equation"],
    ) {
        LawFamily::ReactionStoichiometry
    } else if has_any(
        &text,
        &[
            "entropy",
            "enthalpy",
            "heat capacity",
            "thermodynamic",
            "temperature",
        ],
    ) {
        LawFamily::Thermodynamics
    } else if has_any(
        &text,
        &[
            "electric field",
            "magnetic",
            "maxwell",
            "electrostatic",
            "potential",
        ],
    ) {
        LawFamily::Electromagnetism
    } else if has_any(
        &text,
        &[
            "newton",
            "lagrangian",
            "hamiltonian",
            "momentum",
            "velocity",
        ],
    ) {
        LawFamily::Mechanics
    } else if has_any(
        &text,
        &["quantum", "fermion", "boson", "symmetry class", "spin"],
    ) {
        LawFamily::QuantumPhysics
    } else if has_any(
        &text,
        &[
            "estimator",
            "regression",
            "standard error",
            "confidence interval",
        ],
    ) {
        LawFamily::StatisticalEstimator
    } else if has_any(
        &text,
        &[
            "b cell",
            "t cell",
            "allele",
            "population genetics",
            "genotype",
        ],
    ) {
        LawFamily::PopulationGenetics
    } else if has_any(
        &text,
        &[
            "mutual information",
            "entropy",
            "channel capacity",
            "coding",
        ],
    ) {
        LawFamily::InformationTheory
    } else if has_any(
        &text,
        &[
            "probability",
            "expected value",
            "random variable",
            "distribution",
        ],
    ) {
        LawFamily::ProbabilityFormula
    } else if category.contains("math")
        || has_any(&text, &["polynomial", "matrix", "manifold", "integral"])
    {
        LawFamily::AlgebraicIdentity
    } else {
        LawFamily::OtherSpecialist
    }
}

fn cues(question: &str) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let text = question.to_ascii_lowercase();
    let law_cues = [
        "arrhenius",
        "newton",
        "maxwell",
        "bayes",
        "entropy",
        "thermodynamic",
        "transformer",
        "treatment effect",
        "regression",
        "stoichiometr",
        "kinetic",
        "symmetry",
        "formula",
        "equation",
        "law",
    ]
    .iter()
    .filter(|cue| text.contains(**cue))
    .map(|cue| (*cue).to_string())
    .collect();
    let variables = [
        "x", "y", "z", "t", "n", "m", "d", "p", "q", "r", "v", "u", "lambda", "sigma", "theta",
        "mu", "gamma",
    ]
    .iter()
    .filter(|variable| text.contains(**variable))
    .map(|variable| (*variable).to_string())
    .collect();
    let units = [
        "meter", "second", "kelvin", "joule", "watt", "volt", "ampere", "mole", "gram", "hz", "ev",
        "pascal", "degree",
    ]
    .iter()
    .filter(|unit| text.contains(**unit))
    .map(|unit| (*unit).to_string())
    .collect();
    let assumptions = [
        ("assume", "explicit assumption"),
        ("suppose", "explicit supposition"),
        ("given", "given condition"),
        ("under", "validity condition"),
        ("independent", "independence assumption"),
        ("uniform", "distribution assumption"),
        ("approx", "approximation condition"),
    ]
    .iter()
    .filter(|(term, _)| text.contains(term))
    .map(|(_, label)| (*label).to_string())
    .collect();
    (law_cues, variables, units, assumptions)
}

fn requested_output(question: &str) -> String {
    let text = question.to_ascii_lowercase();
    if has_any(&text, &["how many", "cardinality", "number of", "count"]) {
        "cardinality_or_count".into()
    } else if has_any(
        &text,
        &["which", "choose", "select", "is it true", "yes or no"],
    ) {
        "choice_or_boolean".into()
    } else if has_any(&text, &["formula", "equation", "derive", "expression"]) {
        "equation_or_expression".into()
    } else {
        "scalar_or_structured_value".into()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/hle_knowledge_audit_2147e9e.json".into());
    let output = env::args()
        .nth(2)
        .unwrap_or_else(|| "/tmp/hle_law_audit_2147e9e.json".into());
    let audit_bytes = fs::read(&input)?;
    let audit: KnowledgeAudit = serde_json::from_slice(&audit_bytes)?;
    let dataset_bytes = fs::read(DATASET)?;
    let records = audit
        .records
        .into_iter()
        .filter(|record| record.gap == "missing_equation_or_scientific_law")
        .collect::<Vec<_>>();
    let mut cases = Vec::new();
    for record in records {
        let text = record.question.to_ascii_lowercase();
        let law_family = family(&record.category, &record.question);
        let (law_cues, variables, units, assumptions) = cues(&record.question);
        let output_type = requested_output(&record.question);
        let law_stated = !law_cues.is_empty() || text.contains('=') || text.contains("formula");
        let retrieval_sufficient = law_stated && !assumptions.is_empty() && !law_cues.is_empty();
        let representation = if text.contains('=') || text.contains("formula") {
            vec!["equation_binding".into()]
        } else {
            vec!["named_law_lookup".into()]
        };
        let outcome = if law_family == LawFamily::OtherSpecialist && law_cues.is_empty() {
            AuditOutcome::SpecialistSingleton
        } else if !law_stated {
            AuditOutcome::MissingPrerequisite
        } else if retrieval_sufficient && text.contains("derive") {
            AuditOutcome::DerivationAfterRetrieval
        } else if law_stated && text.contains("definition") {
            AuditOutcome::ConventionOrDefinition
        } else if retrieval_sufficient {
            AuditOutcome::RetrievalReadyEquation
        } else {
            AuditOutcome::InQuestionEquation
        };
        let nearest = match law_family {
            LawFamily::AlgebraicIdentity | LawFamily::ProbabilityFormula => {
                "algebra/formula executor"
            }
            LawFamily::StatisticalEstimator | LawFamily::CausalTreatment => {
                "statistical artifact layer"
            }
            _ => "no authorized domain-law executor",
        };
        cases.push(LawCase {
            id: record.id,
            category: record.category,
            family: law_family,
            law_cues,
            variables,
            units,
            assumptions,
            requested_output: output_type,
            law_stated_in_question: law_stated,
            retrieval_sufficient,
            nearest_existing_capability: nearest.into(),
            bridge_primitives: representation,
            outcome,
        });
    }
    let mut family_map: BTreeMap<LawFamily, Vec<&LawCase>> = BTreeMap::new();
    let mut outcomes = BTreeMap::new();
    let mut family_counts = BTreeMap::new();
    let mut bridge_counts = BTreeMap::new();
    for case in &cases {
        family_map.entry(case.family).or_default().push(case);
        *outcomes.entry(case.outcome).or_insert(0) += 1;
        *family_counts.entry(case.family).or_insert(0) += 1;
        for bridge in &case.bridge_primitives {
            *bridge_counts.entry(bridge.clone()).or_insert(0) += 1;
        }
    }
    let mut families = Vec::new();
    for (law_family, members) in family_map {
        let mut output_types = BTreeMap::new();
        let mut law_cues = BTreeMap::new();
        for member in &members {
            *output_types
                .entry(member.requested_output.clone())
                .or_insert(0) += 1;
            for cue in &member.law_cues {
                *law_cues.entry(cue.clone()).or_insert(0) += 1;
            }
        }
        families.push(FamilySummary {
            family: law_family,
            cases: members.len(),
            case_ids: members.iter().map(|member| member.id.clone()).collect(),
            output_types,
            repeated_law_cues: law_cues,
            retrieval_sufficient_cases: members.iter().filter(|m| m.retrieval_sufficient).count(),
            independent_corpus_required: members.len() >= 2
                && law_family != LawFamily::OtherSpecialist,
        });
    }
    families.sort_by(|left, right| {
        right
            .cases
            .cmp(&left.cases)
            .then_with(|| left.family.cmp(&right.family))
    });
    let report = Report {
        knowledge_audit_sha256: hash(&audit_bytes),
        input_trace_sha256: audit.input_trace_sha256,
        dataset_sha256: hash(&dataset_bytes),
        law_cases: cases.len(),
        outcomes,
        family_counts,
        bridge_primitive_counts: bridge_counts,
        repeated_families: families.iter().filter(|f| f.independent_corpus_required).count(),
        cases,
        families,
        method: "shadow-only law/equation family audit; no retrieval, authorization, or registry mutation".into(),
    };
    fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn law_family_detection_is_bounded() {
        assert_eq!(
            family("Physics", "Use Maxwell equation for electric field"),
            LawFamily::Electromagnetism
        );
        assert_eq!(
            family("Chemistry", "Arrhenius reaction rate"),
            LawFamily::ChemicalKinetics
        );
    }

    #[test]
    fn retrieval_requires_law_and_conditions() {
        let text = "Use a formula to derive x.";
        let (laws, _, _, assumptions) = cues(text);
        assert!(!laws.is_empty());
        assert!(assumptions.is_empty());
    }
}
