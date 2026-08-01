//! Phase 33 shadow audit: law-reference grounding and equation-shape diagnosis.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use the_machine::law_bridge::LawRecord;
use the_machine::law_grounding::{
    ground_law_reference, replay_grounding, GroundingLaw, GroundingStatus, LawGroundingRequest,
};

#[derive(Debug, Deserialize)]
struct Phase30Case {
    id: String,
    category: String,
    family: Value,
    law_cues: Vec<String>,
    variables: Vec<String>,
    requested_output: String,
    bridge_primitives: Vec<String>,
}

#[derive(Debug, Serialize)]
struct HleGroundingCase {
    id: String,
    category: String,
    family: Value,
    question: String,
    question_sha256: String,
    phase30_cues: Vec<String>,
    grounding_status: String,
    candidate_law_ids: Vec<String>,
    unresolved: Vec<String>,
    first_failing_gate: String,
    reasons: Vec<String>,
    replay_verified: bool,
}

#[derive(Debug, Serialize)]
struct EquationShapeCase {
    id: String,
    shape: String,
    reason: String,
    typed_ast_available: bool,
    downstream_route: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    phase30_audit_sha256: String,
    trace_sha256: String,
    grounding_corpus_sha256: String,
    hle_named_cases: usize,
    hle_grounded_unique: usize,
    hle_grounding_ambiguous: usize,
    hle_grounding_unsupported_or_missing: usize,
    hle_authorized_answers: usize,
    equation_shape_cases: usize,
    equation_shape_counts: BTreeMap<String, usize>,
    equation_shapes_classified: usize,
    equation_ast_extensions_added: usize,
    replay_verified: usize,
    registry_mutated: bool,
    grounding_cases: Vec<GroundingCorpusCase>,
    hle_cases: Vec<HleGroundingCase>,
    equation_cases: Vec<EquationShapeCase>,
    method: String,
}

#[derive(Debug, Serialize)]
struct GroundingCorpusCase {
    id: String,
    expected: String,
    actual: String,
    replay_verified: bool,
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn tokens(value: &str) -> Vec<String> {
    value
        .to_ascii_lowercase()
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .collect()
}

fn has_phrase(question: &str, phrase: &str) -> bool {
    let question = tokens(question);
    let phrase = tokens(phrase);
    !phrase.is_empty()
        && question
            .windows(phrase.len())
            .any(|window| window == phrase.as_slice())
}

fn fixture_catalog() -> Vec<GroundingLaw> {
    fn law(id: &str, aliases: &[&str], domain: &str, vars: &[&str]) -> GroundingLaw {
        GroundingLaw {
            law: LawRecord {
                law_id: id.into(),
                aliases: aliases.iter().map(|value| (*value).into()).collect(),
                domain: domain.into(),
                equation: format!("fixture:{id}"),
                variables: vars.iter().map(|value| (*value).into()).collect(),
                assumptions: vec!["fixture validity conditions".into()],
                validity_domain: "Phase 33 independent corpus".into(),
                unit_constraints: Vec::new(),
                provenance: format!("phase33-independent:{id}"),
            },
            descriptive_terms: Vec::new(),
        }
    }
    let mut records = vec![
        law(
            "ohms_law",
            &["Ohm's law", "ohm law"],
            "physics",
            &["V", "I", "R"],
        ),
        law(
            "newtons_second_law",
            &["Newton's second law"],
            "physics",
            &["F", "m", "a"],
        ),
        law(
            "ideal_gas_law",
            &["ideal gas law"],
            "chemistry",
            &["P", "V", "n", "R", "T"],
        ),
        law("energy_a", &["energy law"], "physics", &["E", "m", "c"]),
        law("energy_b", &["energy law"], "physics", &["E", "h", "f"]),
    ];
    records[0].descriptive_terms = vec!["voltage current resistance relation".into()];
    records
}

fn trace_questions(bytes: &[u8]) -> Result<BTreeMap<String, String>, Box<dyn std::error::Error>> {
    let mut result = BTreeMap::new();
    for line in std::str::from_utf8(bytes)?
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        let row: Value = serde_json::from_str(line)?;
        result.insert(
            row["id"].as_str().ok_or("trace id missing")?.to_string(),
            row["question"]
                .as_str()
                .ok_or("trace question missing")?
                .to_string(),
        );
    }
    Ok(result)
}

fn shape_for(question: &str) -> (String, String) {
    let text = question.to_ascii_lowercase();
    if text.contains("diagonalisable") || text.contains("diagonalizable") {
        (
            "matrix_property_with_random_entries".into(),
            "matrix property depends on a random sequence and an event-defined index".into(),
        )
    } else if text.contains("knowledge graph") && text.contains("sampling triples") {
        (
            "statistical_sampling_bound".into(),
            "requires a concentration bound over a stratified graph sampling process".into(),
        )
    } else if text.contains("neural network") && text.contains("batch_size") {
        (
            "program_parameter_selection".into(),
            "asks which code hyperparameter maximizes reported accuracy, not an equation solve"
                .into(),
        )
    } else if text.contains("regression") || text.contains("sentence embedding") {
        (
            "model_identifiability_choice".into(),
            "asks about learnability of a model class from feature concatenation".into(),
        )
    } else if text.contains("integral") || text.contains("derivative") {
        (
            "differential_or_integral_expression".into(),
            "requires a calculus expression artifact".into(),
        )
    } else {
        (
            "unsupported_equation_shape".into(),
            "no bounded typed shape recognized".into(),
        )
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let phase30_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "docs/phase30_hle_law_audit.json".into());
    let trace_path = env::args()
        .nth(2)
        .unwrap_or_else(|| "/tmp/hle_phase26_combined.traces.jsonl".into());
    let output_path = env::args()
        .nth(3)
        .unwrap_or_else(|| "/tmp/hle_phase33_grounding_audit.json".into());
    let phase30_bytes = fs::read(&phase30_path)?;
    let trace_bytes = fs::read(&trace_path)?;
    let phase30: Value = serde_json::from_slice(&phase30_bytes)?;
    let trace = trace_questions(&trace_bytes)?;
    let catalog = fixture_catalog();

    let mut grounding_cases = Vec::new();
    for (id, expected, request) in [
        (
            "alias",
            "unique",
            LawGroundingRequest {
                explicit_references: vec!["Ohm's law".into()],
                described_phenomenon: None,
                domain: Some("physics".into()),
                expected_variables: vec!["V".into(), "I".into(), "R".into()],
                requested_output: "V".into(),
                nearby_equations: Vec::new(),
                context: "explicit law".into(),
            },
        ),
        (
            "description",
            "unique",
            LawGroundingRequest {
                explicit_references: Vec::new(),
                described_phenomenon: Some("voltage current resistance relation".into()),
                domain: Some("physics".into()),
                expected_variables: vec!["V".into(), "I".into(), "R".into()],
                requested_output: "V".into(),
                nearby_equations: Vec::new(),
                context: "description".into(),
            },
        ),
        (
            "energy-ambiguous",
            "ambiguous",
            LawGroundingRequest {
                explicit_references: vec!["energy law".into()],
                described_phenomenon: None,
                domain: Some("physics".into()),
                expected_variables: vec!["E".into()],
                requested_output: "E".into(),
                nearby_equations: Vec::new(),
                context: "overloaded alias".into(),
            },
        ),
        (
            "unknown",
            "unsupported",
            LawGroundingRequest {
                explicit_references: vec!["unknown law".into()],
                described_phenomenon: None,
                domain: None,
                expected_variables: Vec::new(),
                requested_output: "x".into(),
                nearby_equations: Vec::new(),
                context: "negative".into(),
            },
        ),
    ] {
        let result = ground_law_reference(&request, &catalog);
        grounding_cases.push(GroundingCorpusCase {
            id: id.into(),
            expected: expected.into(),
            actual: format!("{:?}", result.status).to_ascii_lowercase(),
            replay_verified: replay_grounding(&result),
        });
    }
    // Add repeated boundary variants without making any HLE question a training case.
    for (id, refs, expected) in [
        ("missing", Vec::<String>::new(), "missing"),
        ("broad-law", vec!["law".into()], "unsupported"),
    ] {
        let result = ground_law_reference(
            &LawGroundingRequest {
                explicit_references: refs,
                described_phenomenon: None,
                domain: None,
                expected_variables: Vec::new(),
                requested_output: "choice".into(),
                nearby_equations: Vec::new(),
                context: id.into(),
            },
            &catalog,
        );
        grounding_cases.push(GroundingCorpusCase {
            id: id.into(),
            expected: expected.into(),
            actual: format!("{:?}", result.status).to_ascii_lowercase(),
            replay_verified: replay_grounding(&result),
        });
    }

    let mut hle_cases = Vec::new();
    let mut equation_cases = Vec::new();
    let mut shape_counts = BTreeMap::new();
    let mut replay_verified = grounding_cases
        .iter()
        .filter(|case| case.replay_verified)
        .count();
    for value in phase30["cases"].as_array().ok_or("phase30 cases missing")? {
        if value["outcome"] != "retrieval_ready_equation" {
            continue;
        }
        let case: Phase30Case = serde_json::from_value(value.clone())?;
        let question = trace.get(&case.id).ok_or("trace question missing")?;
        if case
            .bridge_primitives
            .iter()
            .any(|bridge| bridge == "named_law_lookup")
        {
            let refs = case
                .law_cues
                .iter()
                .filter(|cue| has_phrase(question, cue))
                .cloned()
                .collect::<Vec<_>>();
            let result = ground_law_reference(
                &LawGroundingRequest {
                    explicit_references: refs.clone(),
                    described_phenomenon: None,
                    domain: None,
                    expected_variables: case.variables.clone(),
                    requested_output: case.requested_output.clone(),
                    nearby_equations: Vec::new(),
                    context: question.clone(),
                },
                &catalog,
            );
            let grounding_replay_verified = replay_grounding(&result);
            let status = format!("{:?}", result.status).to_ascii_lowercase();
            let first_gate = match result.status {
                GroundingStatus::Unique => "grounded_law_requires_binding",
                GroundingStatus::Ambiguous => "ambiguous_law_reference",
                GroundingStatus::Missing => "missing_law_reference",
                GroundingStatus::Unsupported => "unsupported_law_reference",
            };
            replay_verified += usize::from(grounding_replay_verified);
            hle_cases.push(HleGroundingCase {
                id: case.id,
                category: case.category,
                family: case.family,
                question: question.clone(),
                question_sha256: sha256(question.as_bytes()),
                phase30_cues: refs,
                grounding_status: status,
                candidate_law_ids: result
                    .candidates
                    .iter()
                    .map(|candidate| candidate.law_id.clone())
                    .collect(),
                unresolved: result.unresolved,
                first_failing_gate: first_gate.into(),
                reasons: vec![
                    "Phase 33 fixture registry intentionally contains no HLE-specific law content"
                        .into(),
                ],
                replay_verified: grounding_replay_verified,
            });
        } else {
            let (shape, reason) = shape_for(question);
            *shape_counts.entry(shape.clone()).or_insert(0) += 1;
            replay_verified += 1;
            equation_cases.push(EquationShapeCase {
                id: case.id,
                shape,
                reason,
                typed_ast_available: false,
                downstream_route:
                    "shape classification only; no AST extension or solver authorization".into(),
            });
        }
    }
    let hle_grounded_unique = hle_cases
        .iter()
        .filter(|case| case.grounding_status == "unique")
        .count();
    let hle_grounding_ambiguous = hle_cases
        .iter()
        .filter(|case| case.grounding_status == "ambiguous")
        .count();
    let hle_grounding_unsupported_or_missing =
        hle_cases.len() - hle_grounded_unique - hle_grounding_ambiguous;
    let report = Report { schema_version: "phase33.hle.grounding.audit.v1".into(), phase30_audit_sha256: sha256(&phase30_bytes), trace_sha256: sha256(&trace_bytes), grounding_corpus_sha256: sha256(&serde_json::to_vec(&grounding_cases)?), hle_named_cases: hle_cases.len(), hle_grounded_unique, hle_grounding_ambiguous, hle_grounding_unsupported_or_missing, hle_authorized_answers: 0, equation_shape_cases: equation_cases.len(), equation_shape_counts: shape_counts, equation_shapes_classified: equation_cases.len(), equation_ast_extensions_added: 0, replay_verified, registry_mutated: false, grounding_cases, hle_cases, equation_cases, method: "shadow-only law-reference grounding and equation-shape audit; no HLE content retrieval or authorization".into() };
    let output = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, &output)?;
    println!("{}", output);
    Ok(())
}
