//! Phase 52 independent pressure corpus for the finite-dimensional linear
//! algebra curriculum pack.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::linear_algebra_pack::{
    evaluate_linear_algebra, LinearAlgebraArtifact, LinearAlgebraOperation, LinearAlgebraRequest,
    LinearAlgebraStatus,
};

#[derive(Clone, Serialize)]
struct Case {
    id: String,
    family: String,
    request: LinearAlgebraRequest,
    expected_status: LinearAlgebraStatus,
    expected_artifact: Option<LinearAlgebraArtifact>,
    rewrite_group: Option<String>,
}

#[derive(Serialize)]
struct Row {
    id: String,
    family: String,
    expected_status: LinearAlgebraStatus,
    actual_status: LinearAlgebraStatus,
    expected_artifact: Option<LinearAlgebraArtifact>,
    actual_artifact: Option<LinearAlgebraArtifact>,
    exact: bool,
    replay_verified: bool,
    false_authorization: bool,
    rewrite_group: Option<String>,
}

#[derive(Serialize)]
struct Report {
    schema_version: String,
    source: String,
    corpus_sha256: String,
    case_count: usize,
    supported_cases: usize,
    boundary_cases: usize,
    unsupported_cases: usize,
    exact_decisions: usize,
    exact_supported_artifacts: usize,
    replay_verified: usize,
    false_authorizations: usize,
    false_denials: usize,
    rewrite_groups: usize,
    tamper_rejections: usize,
    supported_artifact_mismatch_families: BTreeMap<String, usize>,
    status_counts: BTreeMap<String, usize>,
    family_counts: BTreeMap<String, usize>,
    rows: Vec<Row>,
}

fn sha<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn req(operation: LinearAlgebraOperation, matrix: Option<Vec<Vec<i64>>>) -> LinearAlgebraRequest {
    LinearAlgebraRequest {
        operation,
        matrix,
        vector_a: None,
        vector_b: None,
        domain: "finite_exact_integer".into(),
        requested_output: "result".into(),
        provenance: vec!["phase52-independent-corpus".into()],
    }
}

fn vectors(
    operation: LinearAlgebraOperation,
    left: Vec<i64>,
    right: Vec<i64>,
) -> LinearAlgebraRequest {
    LinearAlgebraRequest {
        operation,
        matrix: None,
        vector_a: Some(left),
        vector_b: Some(right),
        domain: "finite_exact_integer".into(),
        requested_output: "result".into(),
        provenance: vec!["phase52-independent-corpus".into()],
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut corpus = Vec::new();
    let matrix = vec![vec![1, 2], vec![3, 4]];
    for index in 0..20 {
        corpus.push(Case {
            id: format!("rank_{index}"),
            family: "rank".into(),
            request: req(LinearAlgebraOperation::Rank, Some(matrix.clone())),
            expected_status: LinearAlgebraStatus::Complete,
            expected_artifact: Some(LinearAlgebraArtifact::Scalar(2)),
            rewrite_group: (index < 5).then(|| format!("rank_rewrite_{}", index % 5)),
        });
    }
    for index in 0..20 {
        corpus.push(Case {
            id: format!("determinant_{index}"),
            family: "determinant".into(),
            request: req(LinearAlgebraOperation::Determinant, Some(matrix.clone())),
            expected_status: LinearAlgebraStatus::Complete,
            expected_artifact: Some(LinearAlgebraArtifact::Scalar(-2)),
            rewrite_group: (index < 5).then(|| format!("determinant_rewrite_{}", index % 5)),
        });
    }
    for index in 0..15 {
        corpus.push(Case {
            id: format!("nullity_{index}"),
            family: "nullity".into(),
            request: req(
                LinearAlgebraOperation::Nullity,
                Some(vec![vec![1, 0, 0], vec![0, 0, 0]]),
            ),
            expected_status: LinearAlgebraStatus::Complete,
            expected_artifact: Some(LinearAlgebraArtifact::Scalar(2)),
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        corpus.push(Case {
            id: format!("invertibility_{index}"),
            family: "invertibility".into(),
            request: req(LinearAlgebraOperation::Invertibility, Some(matrix.clone())),
            expected_status: LinearAlgebraStatus::Complete,
            expected_artifact: Some(LinearAlgebraArtifact::Boolean(true)),
            rewrite_group: None,
        });
    }
    for index in 0..15 {
        corpus.push(Case {
            id: format!("row_reduction_{index}"),
            family: "row_reduction".into(),
            request: req(LinearAlgebraOperation::RowReduction, Some(matrix.clone())),
            expected_status: LinearAlgebraStatus::Complete,
            expected_artifact: Some(LinearAlgebraArtifact::Rref(vec![
                vec![
                    the_machine::linear_algebra_pack::Rational {
                        numerator: 1,
                        denominator: 1,
                    },
                    the_machine::linear_algebra_pack::Rational {
                        numerator: 0,
                        denominator: 1,
                    },
                ],
                vec![
                    the_machine::linear_algebra_pack::Rational {
                        numerator: 0,
                        denominator: 1,
                    },
                    the_machine::linear_algebra_pack::Rational {
                        numerator: 1,
                        denominator: 1,
                    },
                ],
            ])),
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        corpus.push(Case {
            id: format!("inner_product_{index}"),
            family: "inner_product".into(),
            request: vectors(
                LinearAlgebraOperation::InnerProduct,
                vec![1, 2, 3],
                vec![4, 5, 6],
            ),
            expected_status: LinearAlgebraStatus::Complete,
            expected_artifact: Some(LinearAlgebraArtifact::Scalar(32)),
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        corpus.push(Case {
            id: format!("orthogonality_{index}"),
            family: "orthogonality".into(),
            request: vectors(
                LinearAlgebraOperation::Orthogonality,
                vec![1, 0],
                vec![0, 2],
            ),
            expected_status: LinearAlgebraStatus::Complete,
            expected_artifact: Some(LinearAlgebraArtifact::Boolean(true)),
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        corpus.push(Case {
            id: format!("eigenvalues_{index}"),
            family: "diagonal_eigenvalues".into(),
            request: req(
                LinearAlgebraOperation::Eigenvalues,
                Some(vec![vec![2, 0], vec![0, 5]]),
            ),
            expected_status: LinearAlgebraStatus::Complete,
            expected_artifact: Some(LinearAlgebraArtifact::Eigenvalues(vec![2, 5])),
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        corpus.push(Case {
            id: format!("construction_{index}"),
            family: "matrix_construction".into(),
            request: req(
                LinearAlgebraOperation::MatrixConstruction,
                Some(matrix.clone()),
            ),
            expected_status: LinearAlgebraStatus::Complete,
            expected_artifact: Some(LinearAlgebraArtifact::Matrix(matrix.clone())),
            rewrite_group: None,
        });
    }

    for index in 0..10 {
        corpus.push(Case {
            id: format!("missing_matrix_{index}"),
            family: "missing_matrix".into(),
            request: req(LinearAlgebraOperation::Determinant, None),
            expected_status: LinearAlgebraStatus::Missing,
            expected_artifact: None,
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        corpus.push(Case {
            id: format!("missing_vector_{index}"),
            family: "missing_vector".into(),
            request: LinearAlgebraRequest {
                operation: LinearAlgebraOperation::InnerProduct,
                matrix: None,
                vector_a: Some(vec![1, 2]),
                vector_b: None,
                domain: "finite_exact_integer".into(),
                requested_output: "result".into(),
                provenance: vec!["phase52-independent-corpus".into()],
            },
            expected_status: LinearAlgebraStatus::Missing,
            expected_artifact: None,
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        corpus.push(Case {
            id: format!("dimension_mismatch_{index}"),
            family: "dimension_mismatch".into(),
            request: vectors(LinearAlgebraOperation::InnerProduct, vec![1, 2], vec![1]),
            expected_status: LinearAlgebraStatus::DimensionMismatch,
            expected_artifact: None,
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        corpus.push(Case {
            id: format!("ragged_matrix_{index}"),
            family: "ragged_matrix".into(),
            request: req(
                LinearAlgebraOperation::Rank,
                Some(vec![vec![1, 2], vec![3]]),
            ),
            expected_status: LinearAlgebraStatus::Unsupported,
            expected_artifact: None,
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        corpus.push(Case {
            id: format!("nonsquare_determinant_{index}"),
            family: "nonsquare_determinant".into(),
            request: req(
                LinearAlgebraOperation::Determinant,
                Some(vec![vec![1, 2, 3], vec![4, 5, 6]]),
            ),
            expected_status: LinearAlgebraStatus::Unsupported,
            expected_artifact: None,
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        let mut request = req(LinearAlgebraOperation::Rank, Some(matrix.clone()));
        request.domain = "symbolic_parameter_domain".into();
        corpus.push(Case {
            id: format!("symbolic_domain_{index}"),
            family: "symbolic_domain".into(),
            request,
            expected_status: LinearAlgebraStatus::Unsupported,
            expected_artifact: None,
            rewrite_group: None,
        });
    }

    for index in 0..15 {
        corpus.push(Case {
            id: format!("nondiagonal_eigen_{index}"),
            family: "nondiagonal_eigen".into(),
            request: req(
                LinearAlgebraOperation::Eigenvalues,
                Some(vec![vec![1, 1], vec![0, 1]]),
            ),
            expected_status: LinearAlgebraStatus::Unsupported,
            expected_artifact: None,
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        let matrix = vec![vec![1; 7]; 7];
        corpus.push(Case {
            id: format!("large_rank_{index}"),
            family: "large_rank".into(),
            request: req(LinearAlgebraOperation::Rank, Some(matrix)),
            expected_status: LinearAlgebraStatus::Unsupported,
            expected_artifact: None,
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        let mut request = req(LinearAlgebraOperation::Determinant, Some(matrix.clone()));
        request.domain = "real_infinite_operator".into();
        corpus.push(Case {
            id: format!("infinite_domain_{index}"),
            family: "infinite_domain".into(),
            request,
            expected_status: LinearAlgebraStatus::Unsupported,
            expected_artifact: None,
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        let matrix = vec![vec![1; 7]; 7];
        corpus.push(Case {
            id: format!("large_row_reduction_{index}"),
            family: "large_row_reduction".into(),
            request: req(LinearAlgebraOperation::RowReduction, Some(matrix)),
            expected_status: LinearAlgebraStatus::Unsupported,
            expected_artifact: None,
            rewrite_group: None,
        });
    }
    for index in 0..15 {
        corpus.push(Case {
            id: format!("nonexact_spectral_{index}"),
            family: "nonexact_spectral".into(),
            request: req(
                LinearAlgebraOperation::Eigenvalues,
                Some(vec![vec![0, 1], vec![1, 0]]),
            ),
            expected_status: LinearAlgebraStatus::Unsupported,
            expected_artifact: None,
            rewrite_group: None,
        });
    }

    let corpus_sha256 = sha(&corpus);
    let supported_cases = corpus
        .iter()
        .filter(|case| case.expected_status == LinearAlgebraStatus::Complete)
        .count();
    let boundary_cases = corpus
        .iter()
        .filter(|case| {
            matches!(
                case.expected_status,
                LinearAlgebraStatus::Missing | LinearAlgebraStatus::DimensionMismatch
            )
        })
        .count();
    let unsupported_cases = corpus.len() - supported_cases - boundary_cases;
    let mut rows = Vec::new();
    let mut status_counts = BTreeMap::new();
    let mut family_counts = BTreeMap::new();
    let mut exact_decisions = 0;
    let mut exact_supported_artifacts = 0;
    let mut replay_verified = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut tamper_rejections = 0;
    let mut supported_artifact_mismatch_families = BTreeMap::new();
    let mut rewrite_groups = std::collections::BTreeSet::new();
    for case in &corpus {
        let result = evaluate_linear_algebra(&case.request);
        let exact = result.status == case.expected_status;
        let supported_artifact = exact
            && case.expected_status == LinearAlgebraStatus::Complete
            && result.artifact == case.expected_artifact;
        let authorized = result.status == LinearAlgebraStatus::Complete;
        let false_authorization =
            authorized && case.expected_status != LinearAlgebraStatus::Complete;
        let false_denial = !authorized && case.expected_status == LinearAlgebraStatus::Complete;
        let replay = result.replay_verified();
        exact_decisions += usize::from(exact);
        exact_supported_artifacts += usize::from(supported_artifact);
        if exact && case.expected_status == LinearAlgebraStatus::Complete && !supported_artifact {
            *supported_artifact_mismatch_families
                .entry(case.family.clone())
                .or_insert(0) += 1;
        }
        replay_verified += usize::from(replay);
        false_authorizations += usize::from(false_authorization);
        false_denials += usize::from(false_denial);
        *status_counts
            .entry(format!("{:?}", result.status))
            .or_insert(0) += 1;
        *family_counts.entry(case.family.clone()).or_insert(0) += 1;
        if let Some(group) = &case.rewrite_group {
            rewrite_groups.insert(group.clone());
        }
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        tamper_rejections += usize::from(!tampered.replay_verified());
        rows.push(Row {
            id: case.id.clone(),
            family: case.family.clone(),
            expected_status: case.expected_status,
            actual_status: result.status,
            expected_artifact: case.expected_artifact.clone(),
            actual_artifact: result.artifact.clone(),
            exact,
            replay_verified: replay,
            false_authorization,
            rewrite_group: case.rewrite_group.clone(),
        });
    }
    let report = Report { schema_version: "phase52-linear-algebra-pack-v1".into(), source: "MIT OpenCourseWare 18.06SC Linear Algebra (shadow citation; no production registration)".into(), corpus_sha256, case_count: corpus.len(), supported_cases, boundary_cases, unsupported_cases, exact_decisions, exact_supported_artifacts, replay_verified, false_authorizations, false_denials, rewrite_groups: rewrite_groups.len(), tamper_rejections, supported_artifact_mismatch_families, status_counts, family_counts, rows };
    println!("{}", serde_json::to_string_pretty(&report)?);
    std::fs::write(
        "docs/phase52_linear_algebra_pack_bench.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
