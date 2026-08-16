//! Bounded exact spectral linear algebra.
//!
//! This module is deliberately separate from the foundational linear-algebra
//! pack.  It supports small integer matrices, exact characteristic
//! polynomials, integer-root eigenspaces, bounded matrix powers, and explicit
//! diagonalizable decompositions.  Irrational roots, approximate spectra,
//! infinite-dimensional operators, and unbounded powers remain closed.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAX_DIM: usize = 4;
const MAX_POWER: u32 = 8;
const ROOT_BOUND: i64 = 64;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpectralOperation {
    CharacteristicPolynomial,
    IntegerEigenvalues,
    Eigenspace,
    Diagonalizability,
    MatrixPower,
    SpectralDecomposition,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpectralStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidDomain,
    Inconsistent,
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
        let divisor = gcd_i128(numerator.abs(), denominator);
        Some(Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SpectralArtifact {
    CharacteristicPolynomial(Vec<i128>),
    Eigenvalues(Vec<i64>),
    Eigenspace {
        eigenvalue: i64,
        basis: Vec<Vec<Rational>>,
    },
    Diagonalizable(bool),
    Matrix(Vec<Vec<i128>>),
    Decomposition {
        eigenvalues: Vec<i64>,
        basis: Vec<Vec<Rational>>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpectralRequest {
    pub operation: SpectralOperation,
    pub matrix: Option<Vec<Vec<i64>>>,
    pub eigenvalue: Option<i64>,
    pub power: Option<u32>,
    pub domain: String,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpectralResult {
    pub status: SpectralStatus,
    pub artifact: Option<SpectralArtifact>,
    pub operation: SpectralOperation,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn payload(result: &SpectralResult) -> impl Serialize + '_ {
    (
        result.status,
        result.artifact.as_ref(),
        result.operation,
        &result.assumptions,
        &result.reasons,
        &result.provenance,
    )
}

fn result(
    request: &SpectralRequest,
    status: SpectralStatus,
    artifact: Option<SpectralArtifact>,
    assumptions: Vec<String>,
    reasons: Vec<String>,
) -> SpectralResult {
    let mut output = SpectralResult {
        status,
        artifact,
        operation: request.operation,
        assumptions,
        reasons,
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&output));
    output.replay_hash = replay_hash;
    output
}

fn gcd_i128(mut left: i128, mut right: i128) -> i128 {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left.max(1)
}

fn shape(matrix: &[Vec<i64>]) -> Option<usize> {
    let n = matrix.len();
    (n > 0 && n <= MAX_DIM && matrix.iter().all(|row| row.len() == n)).then_some(n)
}

fn determinant_poly(matrix: &[Vec<i64>]) -> Option<Vec<i128>> {
    let n = shape(matrix)?;
    // Build det(t I - A) by permutation expansion. n <= 4 keeps the exact
    // bounded implementation transparent and deterministic.
    let mut output = vec![0i128; n + 1];
    for permutation in permutations(n) {
        let mut inversions = 0;
        for i in 0..n {
            for j in i + 1..n {
                inversions += usize::from(permutation[i] > permutation[j]);
            }
        }
        let sign = if inversions % 2 == 0 { 1i128 } else { -1 };
        let mut factor = vec![1i128];
        for row in 0..n {
            let column = permutation[row];
            let term = if row == column {
                vec![-matrix[row][column] as i128, 1]
            } else {
                vec![-matrix[row][column] as i128]
            };
            factor = poly_mul(&factor, &term);
        }
        for (degree, coefficient) in factor.into_iter().enumerate() {
            output[degree] += sign * coefficient;
        }
    }
    Some(output)
}

fn permutations(n: usize) -> Vec<Vec<usize>> {
    fn visit(start: usize, current: &mut Vec<usize>, output: &mut Vec<Vec<usize>>) {
        if start == current.len() {
            output.push(current.clone());
            return;
        }
        for index in start..current.len() {
            current.swap(start, index);
            visit(start + 1, current, output);
            current.swap(start, index);
        }
    }
    let mut current: Vec<usize> = (0..n).collect();
    let mut output = Vec::new();
    visit(0, &mut current, &mut output);
    output
}

fn poly_mul(left: &[i128], right: &[i128]) -> Vec<i128> {
    let mut output = vec![0; left.len() + right.len() - 1];
    for (i, a) in left.iter().enumerate() {
        for (j, b) in right.iter().enumerate() {
            output[i + j] += a * b;
        }
    }
    output
}

fn poly_eval(polynomial: &[i128], value: i64) -> i128 {
    polynomial.iter().rev().fold(0, |accumulator, coefficient| {
        accumulator * value as i128 + coefficient
    })
}

fn integer_roots(polynomial: &[i128], dimension: usize) -> Vec<i64> {
    let mut roots = Vec::new();
    for candidate in -ROOT_BOUND..=ROOT_BOUND {
        if poly_eval(polynomial, candidate) == 0 {
            roots.push(candidate);
        }
    }
    roots.into_iter().take(dimension).collect()
}

fn divide_by_linear(polynomial: &[i128], root: i64) -> Option<Vec<i128>> {
    if polynomial.len() < 2 || poly_eval(polynomial, root) != 0 {
        return None;
    }
    let mut quotient = vec![0i128; polynomial.len() - 1];
    *quotient.last_mut()? = *polynomial.last()?;
    for index in (0..quotient.len() - 1).rev() {
        quotient[index] = polynomial[index + 1] + root as i128 * quotient[index + 1];
    }
    if polynomial[0] == -root as i128 * quotient[0] {
        Some(quotient)
    } else {
        None
    }
}

fn distinct_integer_roots(polynomial: &[i128], dimension: usize) -> Vec<i64> {
    integer_roots(polynomial, dimension)
}

fn algebraic_multiplicity(polynomial: &[i128], root: i64) -> usize {
    let mut current = polynomial.to_vec();
    let mut count = 0;
    while let Some(quotient) = divide_by_linear(&current, root) {
        count += 1;
        current = quotient;
    }
    count
}

fn rref(mut matrix: Vec<Vec<Rational>>) -> (Vec<Vec<Rational>>, Vec<usize>) {
    let rows = matrix.len();
    let columns = matrix.first().map_or(0, Vec::len);
    let mut pivots = Vec::new();
    let mut pivot_row = 0;
    for column in 0..columns {
        let Some(row) = (pivot_row..rows).find(|row| matrix[*row][column].numerator != 0) else {
            continue;
        };
        matrix.swap(pivot_row, row);
        let pivot = matrix[pivot_row][column].clone();
        for value in &mut matrix[pivot_row] {
            *value = Rational::new(
                value.numerator * pivot.denominator,
                value.denominator * pivot.numerator,
            )
            .unwrap();
        }
        for row in 0..rows {
            if row == pivot_row || matrix[row][column].numerator == 0 {
                continue;
            }
            let factor = matrix[row][column].clone();
            for value in 0..columns {
                let subtraction = Rational::new(
                    factor.numerator * matrix[pivot_row][value].numerator,
                    factor.denominator * matrix[pivot_row][value].denominator,
                )
                .unwrap();
                let current = matrix[row][value].clone();
                *matrix[row].get_mut(value).unwrap() = Rational::new(
                    current.numerator * subtraction.denominator
                        - subtraction.numerator * current.denominator,
                    current.denominator * subtraction.denominator,
                )
                .unwrap();
            }
        }
        pivots.push(column);
        pivot_row += 1;
        if pivot_row == rows {
            break;
        }
    }
    (matrix, pivots)
}

fn eigenspace(matrix: &[Vec<i64>], eigenvalue: i64) -> Option<Vec<Vec<Rational>>> {
    let n = shape(matrix)?;
    let equations = matrix
        .iter()
        .enumerate()
        .map(|(row, values)| {
            values
                .iter()
                .enumerate()
                .map(|(column, value)| {
                    Rational::new(
                        (*value as i128) - if row == column { eigenvalue as i128 } else { 0 },
                        1,
                    )
                    .unwrap()
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let (reduced, pivots) = rref(equations);
    let free: Vec<usize> = (0..n).filter(|column| !pivots.contains(column)).collect();
    let mut basis = Vec::new();
    for free_column in free {
        let mut vector = vec![Rational::new(0, 1).unwrap(); n];
        vector[free_column] = Rational::new(1, 1).unwrap();
        for (row, pivot_column) in pivots.iter().enumerate().rev() {
            vector[*pivot_column] = Rational::new(
                -reduced[row][free_column].numerator,
                reduced[row][free_column].denominator,
            )
            .unwrap();
        }
        basis.push(vector);
    }
    Some(basis)
}

fn matrix_power(matrix: &[Vec<i64>], power: u32) -> Option<Vec<Vec<i128>>> {
    let n = shape(matrix)?;
    let mut output = (0..n)
        .map(|row| (0..n).map(|column| i128::from(row == column)).collect())
        .collect::<Vec<Vec<i128>>>();
    let base = matrix
        .iter()
        .map(|row| row.iter().map(|value| *value as i128).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    for _ in 0..power {
        let mut next = vec![vec![0i128; n]; n];
        for row in 0..n {
            for column in 0..n {
                next[row][column] = (0..n).map(|k| output[row][k] * base[k][column]).sum();
            }
        }
        output = next;
    }
    Some(output)
}

/// Evaluate one bounded exact spectral request.
pub fn evaluate_spectral(request: &SpectralRequest) -> SpectralResult {
    let assumptions = vec![
        "finite integer matrix with dimension at most four".into(),
        "exact arithmetic only; no numerical approximation".into(),
    ];
    if request.domain != "bounded_exact_spectral_linear_algebra" {
        return result(
            request,
            SpectralStatus::InvalidDomain,
            None,
            assumptions,
            vec!["domain outside bounded spectral pack".into()],
        );
    }
    if let Some(ambiguity) = &request.ambiguity {
        return result(
            request,
            SpectralStatus::Ambiguous,
            None,
            assumptions,
            vec![ambiguity.clone()],
        );
    }
    let Some(matrix) = request.matrix.as_deref() else {
        return result(
            request,
            SpectralStatus::Missing,
            None,
            assumptions,
            vec!["matrix is required".into()],
        );
    };
    let Some(dimension) = shape(matrix) else {
        return result(
            request,
            SpectralStatus::Unsupported,
            None,
            assumptions,
            vec!["matrix must be nonempty square and at most four by four".into()],
        );
    };
    let characteristic = determinant_poly(matrix).expect("validated shape");
    let roots = distinct_integer_roots(&characteristic, dimension);
    let algebraic_dimension: usize = roots
        .iter()
        .map(|root| algebraic_multiplicity(&characteristic, *root))
        .sum();
    let expanded_roots = roots
        .iter()
        .flat_map(|root| {
            std::iter::repeat(*root).take(algebraic_multiplicity(&characteristic, *root))
        })
        .collect::<Vec<_>>();
    let complete_integer_spectrum = algebraic_dimension == dimension;
    let artifact = match request.operation {
        SpectralOperation::CharacteristicPolynomial => {
            Some(SpectralArtifact::CharacteristicPolynomial(characteristic))
        }
        SpectralOperation::IntegerEigenvalues => complete_integer_spectrum
            .then_some(SpectralArtifact::Eigenvalues(expanded_roots.clone())),
        SpectralOperation::Eigenspace => {
            let Some(eigenvalue) = request.eigenvalue else {
                return result(
                    request,
                    SpectralStatus::Missing,
                    None,
                    assumptions,
                    vec!["eigenvalue is required".into()],
                );
            };
            if !roots.contains(&eigenvalue) {
                return result(
                    request,
                    SpectralStatus::Inconsistent,
                    None,
                    assumptions,
                    vec!["requested value is not an exact integer root".into()],
                );
            }
            eigenspace(matrix, eigenvalue)
                .map(|basis| SpectralArtifact::Eigenspace { eigenvalue, basis })
        }
        SpectralOperation::Diagonalizability => {
            let total_dimension: usize = roots
                .iter()
                .map(|root| eigenspace(matrix, *root).map_or(0, |basis| basis.len()))
                .sum();
            complete_integer_spectrum.then_some(SpectralArtifact::Diagonalizable(
                total_dimension == dimension,
            ))
        }
        SpectralOperation::MatrixPower => {
            let Some(power) = request.power else {
                return result(
                    request,
                    SpectralStatus::Missing,
                    None,
                    assumptions,
                    vec!["finite power is required".into()],
                );
            };
            if power > MAX_POWER {
                return result(
                    request,
                    SpectralStatus::Unsupported,
                    None,
                    assumptions,
                    vec!["power exceeds bounded exact horizon".into()],
                );
            }
            matrix_power(matrix, power).map(SpectralArtifact::Matrix)
        }
        SpectralOperation::SpectralDecomposition => {
            let mut basis = Vec::new();
            for root in &roots {
                let Some(mut vectors) = eigenspace(matrix, *root) else {
                    continue;
                };
                basis.append(&mut vectors);
            }
            (complete_integer_spectrum && basis.len() == dimension).then_some(
                SpectralArtifact::Decomposition {
                    eigenvalues: expanded_roots.clone(),
                    basis,
                },
            )
        }
    };
    match artifact {
        Some(artifact) => result(request, SpectralStatus::Complete, Some(artifact), assumptions, Vec::new()),
        None => result(request, SpectralStatus::Unsupported, None, assumptions, vec!["the requested exact spectral artifact is outside the bounded integer-root boundary".into()]),
    }
}

impl SpectralResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(operation: SpectralOperation, matrix: Vec<Vec<i64>>) -> SpectralRequest {
        SpectralRequest {
            operation,
            matrix: Some(matrix),
            eigenvalue: None,
            power: None,
            domain: "bounded_exact_spectral_linear_algebra".into(),
            ambiguity: None,
            provenance: vec!["spectral-unit-test".into()],
        }
    }

    #[test]
    fn characteristic_and_eigenspace_are_exact_and_replayable() {
        let matrix = vec![vec![2, 1], vec![1, 2]];
        let characteristic = evaluate_spectral(&request(
            SpectralOperation::CharacteristicPolynomial,
            matrix.clone(),
        ));
        assert_eq!(
            characteristic.artifact,
            Some(SpectralArtifact::CharacteristicPolynomial(vec![3, -4, 1]))
        );
        assert!(characteristic.replay_verified());
        let mut eigenspace_request = request(SpectralOperation::Eigenspace, matrix);
        eigenspace_request.eigenvalue = Some(3);
        let eigenspace = evaluate_spectral(&eigenspace_request);
        assert_eq!(eigenspace.status, SpectralStatus::Complete);
        assert!(eigenspace.replay_verified());
    }
}
