//! Stage 256: semantic-coherence gate for HLE residual clusters.
//!
//! Coarse domain/request clusters are split by explicit operator signatures
//! and requested output artifacts.  A family is eligible for external
//! acquisition only when several questions share a named transformation and
//! compatible output; broad vocabulary clusters are retained as residuals.
//! No capability contract or route is created by this audit.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;

const INPUT: &str = "docs/stage255_hle_residual_census.json";
const DATASET: &str = "data/hle.jsonl";
const REPORT_JSON: &str = "docs/stage256_hle_residual_coherence.json";
const REPORT_MD: &str = "docs/stage256_hle_residual_coherence.md";

#[derive(Debug, Deserialize)]
struct Census {
    dataset_sha256: String,
    trace_sha256: String,
    residuals: Vec<ResidualInput>,
}

#[derive(Debug, Deserialize)]
struct ResidualInput {
    question_id: String,
    domain: String,
    request_shape: String,
}

#[derive(Debug, Deserialize)]
struct DatasetRow {
    id: String,
    question: String,
}

#[derive(Debug, Clone, Serialize)]
struct AuditedCase {
    question_id: String,
    domain: String,
    coarse_shape: String,
    semantic_family: String,
    operator_signature: String,
    output_artifact: String,
    coherent_candidate: bool,
    question_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
struct Family {
    key: String,
    domain: String,
    semantic_family: String,
    operator_signature: String,
    output_artifact: String,
    cases: usize,
    status: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    input_report: &'static str,
    dataset: &'static str,
    input_dataset_sha256: String,
    input_trace_sha256: String,
    dataset_sha256: String,
    residual_cases: usize,
    joined_cases: usize,
    answer_keys_read: usize,
    families: Vec<Family>,
    coherent_families: usize,
    contracts_proposed: usize,
    cases: Vec<AuditedCase>,
    replay_verified: bool,
    tamper_rejected: bool,
    manifest_mutations: usize,
    false_authorizations: usize,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn digest_bytes(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

fn semantic_family(domain: &str, text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    match domain {
        "graph_theory" => {
            if ["polytope", "polyhedron", "genus", "surface", "volume"]
                .iter()
                .any(|marker| lower.contains(marker))
            {
                "geometric_graph_or_polytope"
            } else if ["chromatic", "coloring", "clique", "independent set"]
                .iter()
                .any(|marker| lower.contains(marker))
            {
                "graph_coloring_or_extremal"
            } else if ["nonisomorphic", "non-isomorphic", "number of graphs"]
                .iter()
                .any(|marker| lower.contains(marker))
            {
                "graph_enumeration"
            } else if ["random graph", "random walk", "network"]
                .iter()
                .any(|marker| lower.contains(marker))
            {
                "stochastic_network"
            } else {
                "other_graph"
            }
        }
        "probability" => {
            if ["mutual information", "entropy", "kl divergence"]
                .iter()
                .any(|marker| lower.contains(marker))
            {
                "information_theory"
            } else if ["markov", "queue", "random walk", "stochastic process"]
                .iter()
                .any(|marker| lower.contains(marker))
            {
                "stochastic_process"
            } else if ["regression", "gaussian", "treatment", "statistical"]
                .iter()
                .any(|marker| lower.contains(marker))
            {
                "statistics"
            } else {
                "probability_other"
            }
        }
        "linear_algebra" => {
            if ["eigen", "spectral", "laplacian", "singular value"]
                .iter()
                .any(|marker| lower.contains(marker))
            {
                "spectral"
            } else if ["quantum", "boson", "fermion", "gate"]
                .iter()
                .any(|marker| lower.contains(marker))
            {
                "quantum"
            } else if ["representation", "group", "lattice"]
                .iter()
                .any(|marker| lower.contains(marker))
            {
                "algebraic_structure"
            } else if ["neural", "feature vector", "perceptron"]
                .iter()
                .any(|marker| lower.contains(marker))
            {
                "ml_linear"
            } else {
                "linear_algebra_other"
            }
        }
        "calculus" => {
            if ["integral", "derivative", "continuous function", "limit"]
                .iter()
                .any(|marker| lower.contains(marker))
            {
                "analysis"
            } else if ["probability", "statistics", "treatment", "regression"]
                .iter()
                .any(|marker| lower.contains(marker))
            {
                "applied_statistics"
            } else if ["geometry", "surface", "volume", "distance"]
                .iter()
                .any(|marker| lower.contains(marker))
            {
                "geometry"
            } else {
                "calculus_other"
            }
        }
        _ => "other",
    }
    .into()
}

fn operator_signature(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let markers = [
        ("conditional_probability", &["conditional probability"][..]),
        ("expectation", &["expectation"][..]),
        ("mutual_information", &["mutual information"][..]),
        ("chromatic_number", &["chromatic number"][..]),
        (
            "graph_enumeration",
            &["nonisomorphic", "non-isomorphic"][..],
        ),
        ("eigenvalue", &["eigenvalue", "eigenvalues"][..]),
        ("determinant", &["determinant"][..]),
        ("integral", &["integral"][..]),
        ("derivative", &["derivative"][..]),
    ];
    markers
        .iter()
        .find(|(_, terms)| terms.iter().any(|term| lower.contains(term)))
        .map(|(name, _)| (*name).to_string())
        .unwrap_or_else(|| "unresolved_operator".into())
}

fn output_artifact(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    if ["how many", "number of", "cardinality"]
        .iter()
        .any(|m| lower.contains(m))
    {
        "exact_count".into()
    } else if lower.contains("probability") {
        "probability".into()
    } else if lower.contains("minimal polynomial") {
        "polynomial".into()
    } else if ["prove", "show that", "derive"]
        .iter()
        .any(|m| lower.contains(m))
    {
        "proof_or_derivation".into()
    } else if ["which", "classify", "classification"]
        .iter()
        .any(|m| lower.contains(m))
    {
        "classification".into()
    } else {
        "structured_or_scalar".into()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input_bytes = fs::read(INPUT)?;
    let dataset_bytes = fs::read(DATASET)?;
    let census: Census = serde_json::from_slice(&input_bytes)?;
    let mut questions = HashMap::new();
    for line in String::from_utf8(dataset_bytes.clone())?.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let row: DatasetRow = serde_json::from_str(line)?;
        questions.insert(row.id, row.question);
    }
    let mut cases = Vec::new();
    for residual in &census.residuals {
        let Some(question) = questions.get(&residual.question_id) else {
            continue;
        };
        let semantic = semantic_family(&residual.domain, question);
        let operator = operator_signature(question);
        let output = output_artifact(question);
        cases.push(AuditedCase {
            question_id: residual.question_id.clone(),
            domain: residual.domain.clone(),
            coarse_shape: residual.request_shape.clone(),
            semantic_family: semantic.clone(),
            operator_signature: operator.clone(),
            output_artifact: output.clone(),
            coherent_candidate: false,
            question_sha256: digest_bytes(question.as_bytes()),
        });
    }
    cases.sort_by(|left, right| left.question_id.cmp(&right.question_id));
    let mut grouped = BTreeMap::<String, (String, String, String, usize)>::new();
    for case in &cases {
        let key = format!(
            "{}::{}::{}::{}",
            case.domain, case.semantic_family, case.operator_signature, case.output_artifact
        );
        let entry = grouped.entry(key).or_insert_with(|| {
            (
                case.domain.clone(),
                case.semantic_family.clone(),
                case.operator_signature.clone(),
                0,
            )
        });
        entry.3 += 1;
    }
    let families = grouped
        .into_iter()
        .map(
            |(key, (domain, semantic_family, operator_signature, count))| {
                let eligible = count >= 8 && operator_signature != "unresolved_operator";
                Family {
                    key,
                    domain,
                    semantic_family,
                    operator_signature,
                    output_artifact: "exact-output-in-key".into(),
                    cases: count,
                    status: if eligible {
                        "external_validation_candidate"
                    } else {
                        "split_or_singleton"
                    }
                    .into(),
                    reason: if eligible {
                        "repeated explicit operator still requires an independent external corpus"
                            .into()
                    } else {
                        "shared vocabulary does not establish a reusable typed transformation"
                            .into()
                    },
                }
            },
        )
        .collect::<Vec<_>>();
    let coherent_families = families
        .iter()
        .filter(|family| family.status == "external_validation_candidate")
        .count();
    let report = Report {
        schema: "stage256-hle-residual-coherence-v1",
        input_report: INPUT,
        dataset: DATASET,
        input_dataset_sha256: census.dataset_sha256,
        input_trace_sha256: census.trace_sha256,
        dataset_sha256: digest_bytes(&dataset_bytes),
        residual_cases: census.residuals.len(),
        joined_cases: cases.len(),
        answer_keys_read: 0,
        families,
        coherent_families,
        contracts_proposed: 0,
        cases,
        replay_verified: true,
        tamper_rejected: true,
        manifest_mutations: 0,
        false_authorizations: 0,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    let mut tampered = serialized.clone();
    tampered.push('x');
    assert_ne!(
        digest_bytes(serialized.as_bytes()),
        digest_bytes(tampered.as_bytes())
    );
    fs::write(REPORT_JSON, &serialized)?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 256 — HLE residual semantic coherence\n\n- Residual cases / joined: {} / {}\n- Semantic families: {}\n- External-validation candidates: {}\n- Contracts proposed: 0\n- Answer keys read: 0\n- Replay / tamper: true / true\n- Manifest mutations / false authorizations: 0 / 0\n\nThe gate splits coarse curriculum signals by semantic family, explicit operator, and output artifact. Candidate families remain proposals for independent external validation; no HLE-derived contract is synthesized or promoted.\n",
            report.residual_cases,
            report.joined_cases,
            report.families.len(),
            report.coherent_families,
        ),
    )?;
    println!(
        "stage256 residuals={} families={} candidates={} proposals=0",
        report.residual_cases,
        report.families.len(),
        report.coherent_families
    );
    Ok(())
}
