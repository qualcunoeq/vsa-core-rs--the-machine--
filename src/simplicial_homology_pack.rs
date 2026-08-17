//! Bounded exact simplicial homology over the field `F_2`.
//!
//! This module extends the source-derived finite-topology substrate with a
//! deliberately small, checkable representation of finite simplicial
//! complexes.  It computes only unreduced Betti numbers and Euler
//! characteristics for complexes with at most eight vertices and dimension
//! three.  Coefficients are fixed to `F_2`, so boundary signs and torsion are
//! intentionally outside the contract.

use crate::source_formula_pack::SourceCitation;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const MAX_VERTICES: usize = 8;
const MAX_DIMENSION: usize = 3;
const MAX_SIMPLICES: usize = 64;
const COEFFICIENT_FIELD: u32 = 2;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HomologyOperation {
    ValidateComplex,
    EulerCharacteristic,
    BettiNumbers,
    BoundaryMatrices,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HomologyStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidComplex,
    Inconsistent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimplicialComplexRequest {
    pub operation: HomologyOperation,
    pub domain: String,
    pub vertices: Vec<String>,
    /// Non-empty simplices represented by vertex indices.  Faces are required
    /// explicitly; the empty simplex is not part of this unreduced contract.
    pub simplices: Vec<Vec<usize>>,
    pub coefficient_field: Option<u32>,
    pub provenance: Vec<String>,
    pub ambiguity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HomologyArtifact {
    ValidatedComplex {
        vertices: Vec<String>,
        simplices_by_dimension: Vec<Vec<Vec<usize>>>,
        coefficient_field: u32,
    },
    EulerCharacteristic(i64),
    BettiNumbers(Vec<usize>),
    BoundaryMatrices(Vec<Vec<Vec<u8>>>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HomologyResult {
    pub status: HomologyStatus,
    pub operation: HomologyOperation,
    pub artifact: Option<HomologyArtifact>,
    pub source: SourceCitation,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn source() -> SourceCitation {
    SourceCitation {
        source_id: "topology-without-tears:finite-simplicial-homology".into(),
        title: "Topology Without Tears".into(),
        section: "finite simplicial complexes, boundary maps, and Euler characteristic".into(),
        url: "https://www.topologywithouttears.net/".into(),
        license: "CC BY-NC-SA 4.0; attribution required".into(),
        retrieved_utc: "2026-08-17".into(),
        evidence_span: "finite complexes, boundary maps, Betti numbers, Euler characteristic"
            .into(),
    }
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("homology serializes"))
    )
}

fn payload(result: &HomologyResult) -> impl Serialize + '_ {
    (
        result.status,
        result.operation,
        &result.artifact,
        &result.source,
        &result.assumptions,
        &result.reasons,
        &result.provenance,
    )
}

fn result(
    request: &SimplicialComplexRequest,
    status: HomologyStatus,
    artifact: Option<HomologyArtifact>,
    assumptions: Vec<String>,
    reasons: Vec<String>,
) -> HomologyResult {
    let mut output = HomologyResult {
        status,
        operation: request.operation,
        artifact,
        source: source(),
        assumptions,
        reasons,
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&output));
    output.replay_hash = replay_hash;
    output
}

fn canonical_simplex(simplex: &[usize]) -> Option<Vec<usize>> {
    if simplex.is_empty() {
        return None;
    }
    let mut simplex = simplex.to_vec();
    simplex.sort_unstable();
    if simplex.windows(2).any(|pair| pair[0] == pair[1]) {
        return None;
    }
    Some(simplex)
}

fn faces(simplex: &[usize]) -> Vec<Vec<usize>> {
    (0..simplex.len())
        .map(|removed| {
            simplex
                .iter()
                .enumerate()
                .filter_map(|(index, value)| (index != removed).then_some(*value))
                .collect()
        })
        .collect()
}

fn canonicalize(request: &SimplicialComplexRequest) -> Result<Vec<Vec<Vec<usize>>>, String> {
    if request.domain != "finite_simplicial_complex" {
        return Err("the finite simplicial-complex domain must be explicit".into());
    }
    if request.vertices.is_empty() || request.vertices.len() > MAX_VERTICES {
        return Err(format!("vertex count must be between 1 and {MAX_VERTICES}"));
    }
    let mut names = BTreeSet::new();
    if request.vertices.iter().any(|name| !names.insert(name)) {
        return Err("vertex identities must be unique".into());
    }
    if request.simplices.is_empty() || request.simplices.len() > MAX_SIMPLICES {
        return Err(format!(
            "simplex count must be between 1 and {MAX_SIMPLICES}"
        ));
    }
    let mut seen = BTreeSet::new();
    let mut by_dimension = vec![Vec::new(); MAX_DIMENSION + 1];
    for simplex in &request.simplices {
        if simplex.len() > MAX_DIMENSION + 1 {
            return Err(format!(
                "dimension exceeds the bounded limit {MAX_DIMENSION}"
            ));
        }
        let simplex = canonical_simplex(simplex).ok_or("simplex is empty or repeats a vertex")?;
        if simplex
            .iter()
            .any(|vertex| *vertex >= request.vertices.len())
        {
            return Err("simplex references an unknown vertex".into());
        }
        if !seen.insert(simplex.clone()) {
            return Err("duplicate simplex".into());
        }
        by_dimension[simplex.len() - 1].push(simplex);
    }
    if by_dimension[0].len() != request.vertices.len()
        || by_dimension[0]
            .iter()
            .enumerate()
            .any(|(vertex, simplex)| simplex != &vec![vertex])
    {
        return Err("the complex must explicitly contain every vertex".into());
    }
    for simplices in &mut by_dimension {
        simplices.sort();
    }
    let all = seen;
    for dimension in 1..=MAX_DIMENSION {
        for simplex in &by_dimension[dimension] {
            if faces(simplex).into_iter().any(|face| !all.contains(&face)) {
                return Err("simplicial closure is violated: a face is missing".into());
            }
        }
    }
    Ok(by_dimension)
}

fn boundary_matrices(by_dimension: &[Vec<Vec<usize>>]) -> Vec<Vec<Vec<u8>>> {
    let mut matrices = Vec::with_capacity(by_dimension.len());
    matrices.push(Vec::new());
    for dimension in 1..by_dimension.len() {
        let rows = by_dimension[dimension - 1].len();
        let columns = by_dimension[dimension].len();
        let mut matrix = vec![vec![0u8; columns]; rows];
        let row_index: BTreeMap<Vec<usize>, usize> = by_dimension[dimension - 1]
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, simplex)| (simplex, index))
            .collect();
        for (column, simplex) in by_dimension[dimension].iter().enumerate() {
            for face in faces(simplex) {
                if let Some(row) = row_index.get(&face) {
                    matrix[*row][column] ^= 1;
                }
            }
        }
        matrices.push(matrix);
    }
    matrices
}

fn rank_f2(matrix: &[Vec<u8>]) -> usize {
    if matrix.is_empty() || matrix[0].is_empty() {
        return 0;
    }
    let mut matrix = matrix.to_vec();
    let rows = matrix.len();
    let columns = matrix[0].len();
    let mut pivot_row = 0;
    for column in 0..columns {
        let Some(source_row) = (pivot_row..rows).find(|row| matrix[*row][column] == 1) else {
            continue;
        };
        matrix.swap(pivot_row, source_row);
        for row in 0..rows {
            if row != pivot_row && matrix[row][column] == 1 {
                for entry in column..columns {
                    matrix[row][entry] ^= matrix[pivot_row][entry];
                }
            }
        }
        pivot_row += 1;
        if pivot_row == rows {
            break;
        }
    }
    pivot_row
}

fn boundary_composition_is_zero(matrices: &[Vec<Vec<u8>>]) -> bool {
    for dimension in 2..matrices.len() {
        let first = &matrices[dimension - 1];
        let second = &matrices[dimension];
        if first.is_empty() || second.is_empty() {
            continue;
        }
        let rows = first.len();
        let middle = first[0].len();
        let columns = second[0].len();
        for row in 0..rows {
            for column in 0..columns {
                let mut value = 0u8;
                for index in 0..middle {
                    value ^= first[row][index] & second[index][column];
                }
                if value != 0 {
                    return false;
                }
            }
        }
    }
    true
}

fn betti_numbers(by_dimension: &[Vec<Vec<usize>>], matrices: &[Vec<Vec<u8>>]) -> Vec<usize> {
    (0..by_dimension.len())
        .map(|dimension| {
            let boundary_rank = matrices
                .get(dimension)
                .map(|matrix| rank_f2(matrix))
                .unwrap_or(0);
            let next_rank = matrices
                .get(dimension + 1)
                .map(|matrix| rank_f2(matrix))
                .unwrap_or(0);
            by_dimension[dimension]
                .len()
                .saturating_sub(boundary_rank + next_rank)
        })
        .collect()
}

/// Evaluate a bounded finite simplicial-complex request.
pub fn evaluate(request: &SimplicialComplexRequest) -> HomologyResult {
    if request.ambiguity.is_some() {
        return result(
            request,
            HomologyStatus::Ambiguous,
            None,
            Vec::new(),
            vec!["the request carries an unresolved interpretation".into()],
        );
    }
    match request.coefficient_field {
        None => {
            return result(
                request,
                HomologyStatus::Ambiguous,
                None,
                Vec::new(),
                vec!["the coefficient field is not declared".into()],
            )
        }
        Some(field) if field != COEFFICIENT_FIELD => {
            return result(
                request,
                HomologyStatus::Unsupported,
                None,
                Vec::new(),
                vec!["only unreduced homology over F_2 is supported".into()],
            )
        }
        Some(_) => {}
    }
    let by_dimension = match canonicalize(request) {
        Ok(value) => value,
        Err(reason) => {
            return result(
                request,
                HomologyStatus::InvalidComplex,
                None,
                Vec::new(),
                vec![reason],
            )
        }
    };
    let matrices = boundary_matrices(&by_dimension);
    if !boundary_composition_is_zero(&matrices) {
        return result(
            request,
            HomologyStatus::Inconsistent,
            None,
            vec!["boundary maps must compose to zero".into()],
            vec!["the supplied complex does not form a chain complex".into()],
        );
    }
    let artifact = match request.operation {
        HomologyOperation::ValidateComplex => HomologyArtifact::ValidatedComplex {
            vertices: request.vertices.clone(),
            simplices_by_dimension: by_dimension,
            coefficient_field: COEFFICIENT_FIELD,
        },
        HomologyOperation::EulerCharacteristic => {
            let value =
                by_dimension
                    .iter()
                    .enumerate()
                    .fold(0i64, |sum, (dimension, simplices)| {
                        if dimension % 2 == 0 {
                            sum + simplices.len() as i64
                        } else {
                            sum - simplices.len() as i64
                        }
                    });
            HomologyArtifact::EulerCharacteristic(value)
        }
        HomologyOperation::BettiNumbers => {
            HomologyArtifact::BettiNumbers(betti_numbers(&by_dimension, &matrices))
        }
        HomologyOperation::BoundaryMatrices => HomologyArtifact::BoundaryMatrices(matrices),
    };
    result(
        request,
        HomologyStatus::Complete,
        Some(artifact),
        vec![
            "non-empty simplices only; unreduced homology".into(),
            "coefficients are computed over F_2".into(),
        ],
        Vec::new(),
    )
}

impl HomologyResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != HomologyStatus::Complete || self.artifact.is_some())
    }

    pub fn authorized(&self) -> bool {
        self.status == HomologyStatus::Complete && self.replay_verified()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(
        operation: HomologyOperation,
        simplices: Vec<Vec<usize>>,
    ) -> SimplicialComplexRequest {
        SimplicialComplexRequest {
            operation,
            domain: "finite_simplicial_complex".into(),
            vertices: vec!["a".into(), "b".into(), "c".into()],
            simplices,
            coefficient_field: Some(2),
            provenance: vec!["test".into()],
            ambiguity: None,
        }
    }

    #[test]
    fn filled_triangle_has_one_component_and_zero_cycle() {
        let result = evaluate(&request(
            HomologyOperation::BettiNumbers,
            vec![
                vec![0],
                vec![1],
                vec![2],
                vec![0, 1],
                vec![0, 2],
                vec![1, 2],
                vec![0, 1, 2],
            ],
        ));
        assert_eq!(result.status, HomologyStatus::Complete);
        assert_eq!(
            result.artifact,
            Some(HomologyArtifact::BettiNumbers(vec![1, 0, 0, 0]))
        );
        assert!(result.authorized());
    }

    #[test]
    fn triangle_boundary_has_one_one_cycle() {
        let result = evaluate(&request(
            HomologyOperation::BettiNumbers,
            vec![
                vec![0],
                vec![1],
                vec![2],
                vec![0, 1],
                vec![0, 2],
                vec![1, 2],
            ],
        ));
        assert_eq!(
            result.artifact,
            Some(HomologyArtifact::BettiNumbers(vec![1, 1, 0, 0]))
        );
    }

    #[test]
    fn missing_field_is_ambiguous_and_nonclosed_is_invalid() {
        let mut ambiguous = request(
            HomologyOperation::BettiNumbers,
            vec![vec![0], vec![1], vec![2]],
        );
        ambiguous.coefficient_field = None;
        assert_eq!(evaluate(&ambiguous).status, HomologyStatus::Ambiguous);
        let invalid = request(
            HomologyOperation::BettiNumbers,
            vec![vec![0], vec![1], vec![2], vec![0, 1, 2]],
        );
        assert_eq!(evaluate(&invalid).status, HomologyStatus::InvalidComplex);
    }

    #[test]
    fn tampered_receipt_is_rejected() {
        let mut result = evaluate(&request(
            HomologyOperation::EulerCharacteristic,
            vec![
                vec![0],
                vec![1],
                vec![2],
                vec![0, 1],
                vec![0, 2],
                vec![1, 2],
            ],
        ));
        assert!(result.replay_verified());
        result.replay_hash = "tampered".into();
        assert!(!result.replay_verified());
    }
}
