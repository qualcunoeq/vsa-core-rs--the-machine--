//! Shadow finite-dimensional linear-algebra curriculum pack.
//!
//! This pack intentionally covers exact, bounded foundations only.  It does
//! not interpret specialist notation, infinite-dimensional operators, or
//! theorem-dependent spectral claims.  It is not registered with production
//! routing.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceCitation {
    pub source_id: String,
    pub title: String,
    pub section: String,
    pub url: String,
    pub license: String,
    pub retrieved_utc: String,
    pub evidence_span: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LinearAlgebraOperation {
    VectorConstruction,
    MatrixConstruction,
    Rank,
    Nullity,
    Determinant,
    Invertibility,
    RowReduction,
    Eigenvalues,
    InnerProduct,
    Orthogonality,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LinearAlgebraStatus {
    Complete,
    Missing,
    Ambiguous,
    DimensionMismatch,
    Unsupported,
    Overflow,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Rational {
    pub numerator: i128,
    pub denominator: i128,
}

impl Rational {
    fn new(numerator: i128, denominator: i128) -> Option<Self> {
        if denominator == 0 {
            return None;
        }
        let sign = if denominator < 0 { -1 } else { 1 };
        let numerator = numerator * sign;
        let denominator = denominator.abs();
        let divisor = gcd(numerator.abs(), denominator);
        Some(Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        })
    }

    fn sub(&self, other: &Self) -> Option<Self> {
        Self::new(
            self.numerator * other.denominator - other.numerator * self.denominator,
            self.denominator * other.denominator,
        )
    }

    fn mul(&self, other: &Self) -> Option<Self> {
        Self::new(
            self.numerator * other.numerator,
            self.denominator * other.denominator,
        )
    }

    fn div(&self, other: &Self) -> Option<Self> {
        Self::new(
            self.numerator * other.denominator,
            self.denominator * other.numerator,
        )
    }
}

fn gcd(mut left: i128, mut right: i128) -> i128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.max(1)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LinearAlgebraArtifact {
    Vector(Vec<i64>),
    Matrix(Vec<Vec<i64>>),
    Scalar(i128),
    Boolean(bool),
    Eigenvalues(Vec<i64>),
    Rref(Vec<Vec<Rational>>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinearAlgebraRequest {
    pub operation: LinearAlgebraOperation,
    pub matrix: Option<Vec<Vec<i64>>>,
    pub vector_a: Option<Vec<i64>>,
    pub vector_b: Option<Vec<i64>>,
    pub domain: String,
    pub requested_output: String,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinearAlgebraResult {
    pub status: LinearAlgebraStatus,
    pub artifact: Option<LinearAlgebraArtifact>,
    pub operation: LinearAlgebraOperation,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub source: SourceCitation,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("linear algebra serializes"))
    )
}

fn replay_payload(result: &LinearAlgebraResult) -> impl Serialize + '_ {
    (
        result.status,
        result.artifact.as_ref(),
        result.operation,
        &result.assumptions,
        &result.reasons,
        &result.source,
        &result.provenance,
    )
}

fn compute_replay_hash(result: &LinearAlgebraResult) -> String {
    digest(&replay_payload(result))
}

fn source(section: &str) -> SourceCitation {
    SourceCitation {
        source_id: format!("mit-ocw-18-06sc:{section}"),
        title: "MIT OpenCourseWare 18.06SC Linear Algebra".into(),
        section: section.into(),
        url: "https://ocw.mit.edu/courses/18-06sc-linear-algebra-fall-2011/".into(),
        license: "CC BY-NC-SA 4.0; MIT attribution required".into(),
        retrieved_utc: "2026-08-05".into(),
        evidence_span: format!("{section}: definitions and worked constraints"),
    }
}

fn shape(matrix: &[Vec<i64>]) -> Option<(usize, usize)> {
    let columns = matrix.first()?.len();
    if columns == 0 || matrix.iter().any(|row| row.len() != columns) {
        None
    } else {
        Some((matrix.len(), columns))
    }
}

fn determinant(matrix: &[Vec<i64>]) -> Option<i128> {
    let (rows, columns) = shape(matrix)?;
    if rows != columns || rows > 6 {
        return None;
    }
    if rows == 1 {
        return Some(matrix[0][0] as i128);
    }
    if rows == 2 {
        return Some(
            matrix[0][0] as i128 * matrix[1][1] as i128
                - matrix[0][1] as i128 * matrix[1][0] as i128,
        );
    }
    let mut result = 0i128;
    for column in 0..columns {
        let minor: Vec<Vec<i64>> = matrix[1..]
            .iter()
            .map(|row| {
                row.iter()
                    .enumerate()
                    .filter_map(|(index, value)| (index != column).then_some(*value))
                    .collect()
            })
            .collect();
        let sign = if column % 2 == 0 { 1 } else { -1 };
        result = result.checked_add(sign * matrix[0][column] as i128 * determinant(&minor)?)?;
    }
    Some(result)
}

fn combinations(length: usize, choose: usize) -> Vec<Vec<usize>> {
    fn visit(
        start: usize,
        length: usize,
        choose: usize,
        current: &mut Vec<usize>,
        output: &mut Vec<Vec<usize>>,
    ) {
        if current.len() == choose {
            output.push(current.clone());
            return;
        }
        let remaining = choose - current.len();
        for index in start..=length.saturating_sub(remaining) {
            current.push(index);
            visit(index + 1, length, choose, current, output);
            current.pop();
        }
    }
    let mut output = Vec::new();
    visit(0, length, choose, &mut Vec::new(), &mut output);
    output
}

fn rank(matrix: &[Vec<i64>]) -> Option<usize> {
    let (rows, columns) = shape(matrix)?;
    if rows.max(columns) > 6 {
        return None;
    }
    for size in (1..=rows.min(columns)).rev() {
        for row_indices in combinations(rows, size) {
            for column_indices in combinations(columns, size) {
                let minor: Vec<Vec<i64>> = row_indices
                    .iter()
                    .map(|row| {
                        column_indices
                            .iter()
                            .map(|column| matrix[*row][*column])
                            .collect()
                    })
                    .collect();
                if determinant(&minor) != Some(0) {
                    return Some(size);
                }
            }
        }
    }
    Some(0)
}

fn rref(matrix: &[Vec<i64>]) -> Option<Vec<Vec<Rational>>> {
    let (rows, columns) = shape(matrix)?;
    if rows.max(columns) > 6 {
        return None;
    }
    let mut values: Vec<Vec<Rational>> = matrix
        .iter()
        .map(|row| {
            row.iter()
                .map(|value| Rational::new(*value as i128, 1).unwrap())
                .collect()
        })
        .collect();
    let mut pivot_row = 0;
    for pivot_column in 0..columns {
        let Some(source_row) =
            (pivot_row..rows).find(|row| values[*row][pivot_column].numerator != 0)
        else {
            continue;
        };
        values.swap(pivot_row, source_row);
        let pivot = values[pivot_row][pivot_column].clone();
        for column in 0..columns {
            values[pivot_row][column] = values[pivot_row][column].div(&pivot)?;
        }
        for row in 0..rows {
            if row == pivot_row {
                continue;
            }
            let factor = values[row][pivot_column].clone();
            if factor.numerator == 0 {
                continue;
            }
            for column in 0..columns {
                values[row][column] =
                    values[row][column].sub(&factor.mul(&values[pivot_row][column])?)?;
            }
        }
        pivot_row += 1;
        if pivot_row == rows {
            break;
        }
    }
    Some(values)
}

fn invalid_domain(request: &LinearAlgebraRequest) -> bool {
    request.domain != "finite_exact_integer"
}

pub fn evaluate_linear_algebra(request: &LinearAlgebraRequest) -> LinearAlgebraResult {
    let mut reasons = Vec::new();
    let source = source("foundations-and-computations");
    let assumptions = vec!["finite-dimensional exact integer inputs".into()];
    let mut status = LinearAlgebraStatus::Complete;
    let mut artifact = match request.operation {
        LinearAlgebraOperation::VectorConstruction => {
            request.vector_a.clone().map(LinearAlgebraArtifact::Vector)
        }
        LinearAlgebraOperation::MatrixConstruction => {
            request.matrix.clone().map(LinearAlgebraArtifact::Matrix)
        }
        LinearAlgebraOperation::InnerProduct | LinearAlgebraOperation::Orthogonality => {
            match (&request.vector_a, &request.vector_b) {
                (Some(left), Some(right)) if left.len() == right.len() && !left.is_empty() => {
                    let value = left
                        .iter()
                        .zip(right)
                        .map(|(a, b)| *a as i128 * *b as i128)
                        .sum::<i128>();
                    if request.operation == LinearAlgebraOperation::InnerProduct {
                        Some(LinearAlgebraArtifact::Scalar(value))
                    } else {
                        Some(LinearAlgebraArtifact::Boolean(value == 0))
                    }
                }
                (Some(_), Some(_)) => {
                    status = LinearAlgebraStatus::DimensionMismatch;
                    reasons.push("vector dimensions differ".into());
                    None
                }
                _ => {
                    status = LinearAlgebraStatus::Missing;
                    reasons.push("two vectors are required".into());
                    None
                }
            }
        }
        LinearAlgebraOperation::Rank | LinearAlgebraOperation::Nullity => {
            match request.matrix.as_deref().and_then(rank) {
                Some(value) => {
                    let nullity = request
                        .matrix
                        .as_deref()
                        .and_then(shape)
                        .map(|(_, columns)| columns as i128 - value as i128);
                    Some(LinearAlgebraArtifact::Scalar(
                        if request.operation == LinearAlgebraOperation::Nullity {
                            nullity.unwrap_or_default()
                        } else {
                            value as i128
                        },
                    ))
                }
                None => {
                    status = if request.matrix.is_none() {
                        LinearAlgebraStatus::Missing
                    } else {
                        LinearAlgebraStatus::Unsupported
                    };
                    reasons.push(
                        "matrix is missing, malformed, or exceeds exact bounded rank budget".into(),
                    );
                    None
                }
            }
        }
        LinearAlgebraOperation::Determinant => {
            match request.matrix.as_deref().and_then(determinant) {
                Some(value) => Some(LinearAlgebraArtifact::Scalar(value)),
                None => {
                    status = if request.matrix.is_none() {
                        LinearAlgebraStatus::Missing
                    } else {
                        LinearAlgebraStatus::Unsupported
                    };
                    reasons.push(
                        "determinant requires a square matrix of dimension at most six".into(),
                    );
                    None
                }
            }
        }
        LinearAlgebraOperation::Invertibility => {
            match request.matrix.as_deref().and_then(determinant) {
                Some(value) => Some(LinearAlgebraArtifact::Boolean(value != 0)),
                None => {
                    status = if request.matrix.is_none() {
                        LinearAlgebraStatus::Missing
                    } else {
                        LinearAlgebraStatus::Unsupported
                    };
                    reasons.push("invertibility requires a bounded square exact matrix".into());
                    None
                }
            }
        }
        LinearAlgebraOperation::RowReduction => match request.matrix.as_deref().and_then(rref) {
            Some(value) => Some(LinearAlgebraArtifact::Rref(value)),
            None => {
                status = if request.matrix.is_none() {
                    LinearAlgebraStatus::Missing
                } else {
                    LinearAlgebraStatus::Unsupported
                };
                reasons.push("row reduction exceeds the exact bounded matrix budget".into());
                None
            }
        },
        LinearAlgebraOperation::Eigenvalues => {
            let Some(matrix) = request.matrix.as_deref() else {
                status = LinearAlgebraStatus::Missing;
                reasons.push("matrix is required".into());
                return result(request, status, None, assumptions, reasons, source);
            };
            let Some((rows, columns)) = shape(matrix) else {
                status = LinearAlgebraStatus::DimensionMismatch;
                reasons.push("matrix rows have inconsistent dimensions".into());
                return result(request, status, None, assumptions, reasons, source);
            };
            if rows != columns {
                status = LinearAlgebraStatus::DimensionMismatch;
                reasons.push("eigenvalues require a square matrix".into());
                None
            } else if matrix.iter().enumerate().any(|(row, values)| {
                values
                    .iter()
                    .enumerate()
                    .any(|(column, value)| row != column && *value != 0)
            }) {
                status = LinearAlgebraStatus::Unsupported;
                reasons.push("only diagonal exact eigenvalue extraction is in this pack".into());
                None
            } else {
                Some(LinearAlgebraArtifact::Eigenvalues(
                    (0..rows).map(|index| matrix[index][index]).collect(),
                ))
            }
        }
    };
    if invalid_domain(request) {
        status = LinearAlgebraStatus::Unsupported;
        artifact = None;
        reasons.push("domain is outside finite_exact_integer pack boundary".into());
    }
    result(request, status, artifact, assumptions, reasons, source)
}

fn result(
    request: &LinearAlgebraRequest,
    status: LinearAlgebraStatus,
    artifact: Option<LinearAlgebraArtifact>,
    assumptions: Vec<String>,
    reasons: Vec<String>,
    source: SourceCitation,
) -> LinearAlgebraResult {
    let mut result = LinearAlgebraResult {
        status,
        artifact,
        operation: request.operation,
        assumptions,
        reasons,
        source,
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    let replay_hash = compute_replay_hash(&result);
    result.replay_hash = replay_hash;
    result
}

impl LinearAlgebraResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == compute_replay_hash(self)
            && !self.provenance.is_empty()
            && self.source.source_id.starts_with("mit-ocw-18-06sc:")
            && (self.status != LinearAlgebraStatus::Complete || self.artifact.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(
        operation: LinearAlgebraOperation,
        matrix: Option<Vec<Vec<i64>>>,
    ) -> LinearAlgebraRequest {
        LinearAlgebraRequest {
            operation,
            matrix,
            vector_a: None,
            vector_b: None,
            domain: "finite_exact_integer".into(),
            requested_output: "result".into(),
            provenance: vec!["independent-test".into()],
        }
    }

    #[test]
    fn exact_rank_determinant_and_rref_replay() {
        let matrix = vec![vec![1, 2], vec![3, 4]];
        let determinant = evaluate_linear_algebra(&request(
            LinearAlgebraOperation::Determinant,
            Some(matrix.clone()),
        ));
        assert_eq!(
            determinant.artifact,
            Some(LinearAlgebraArtifact::Scalar(-2))
        );
        assert!(determinant.replay_verified());
        let rank =
            evaluate_linear_algebra(&request(LinearAlgebraOperation::Rank, Some(matrix.clone())));
        assert_eq!(rank.artifact, Some(LinearAlgebraArtifact::Scalar(2)));
        let rref =
            evaluate_linear_algebra(&request(LinearAlgebraOperation::RowReduction, Some(matrix)));
        assert!(matches!(
            rref.artifact,
            Some(LinearAlgebraArtifact::Rref(_))
        ));
    }

    #[test]
    fn boundaries_fail_closed() {
        let mut request = request(
            LinearAlgebraOperation::Eigenvalues,
            Some(vec![vec![1, 1], vec![0, 1]]),
        );
        let result = evaluate_linear_algebra(&request);
        assert_eq!(result.status, LinearAlgebraStatus::Unsupported);
        request.domain = "real_infinite_operator".into();
        let result = evaluate_linear_algebra(&request);
        assert_eq!(result.status, LinearAlgebraStatus::Unsupported);
    }

    #[test]
    fn dimension_mismatch_is_not_authorized() {
        let request = LinearAlgebraRequest {
            operation: LinearAlgebraOperation::InnerProduct,
            matrix: None,
            vector_a: Some(vec![1, 2]),
            vector_b: Some(vec![1]),
            domain: "finite_exact_integer".into(),
            requested_output: "dot".into(),
            provenance: vec!["test".into()],
        };
        let result = evaluate_linear_algebra(&request);
        assert_eq!(result.status, LinearAlgebraStatus::DimensionMismatch);
        assert!(result.artifact.is_none());
    }
}
