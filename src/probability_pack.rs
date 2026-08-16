//! Shadow finite exact probability curriculum pack.
//!
//! The pack covers finite sample spaces and exact rational operations only.
//! It refuses continuous, measure-theoretic, asymptotic, and stochastic-process
//! semantics until those representations have their own curriculum gates.

use crate::linear_algebra_pack::{
    LinearAlgebraArtifact, LinearAlgebraOperation, LinearAlgebraRequest,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Rational {
    pub numerator: i128,
    pub denominator: i128,
}

impl Rational {
    pub fn new(numerator: i128, denominator: i128) -> Option<Self> {
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

    pub fn zero() -> Self {
        Self {
            numerator: 0,
            denominator: 1,
        }
    }
    pub fn one() -> Self {
        Self {
            numerator: 1,
            denominator: 1,
        }
    }
    pub fn add(&self, other: &Self) -> Option<Self> {
        Self::new(
            self.numerator * other.denominator + other.numerator * self.denominator,
            self.denominator * other.denominator,
        )
    }
    pub fn sub(&self, other: &Self) -> Option<Self> {
        Self::new(
            self.numerator * other.denominator - other.numerator * self.denominator,
            self.denominator * other.denominator,
        )
    }
    pub fn mul(&self, other: &Self) -> Option<Self> {
        Self::new(
            self.numerator * other.numerator,
            self.denominator * other.denominator,
        )
    }
    pub fn div(&self, other: &Self) -> Option<Self> {
        Self::new(
            self.numerator * other.denominator,
            self.denominator * other.numerator,
        )
    }
    pub fn nonnegative(&self) -> bool {
        self.numerator >= 0
    }
    pub fn positive(&self) -> bool {
        self.numerator > 0
    }
    pub fn in_unit_interval(&self) -> bool {
        self.nonnegative() && self.numerator <= self.denominator
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
pub struct FiniteDistribution {
    pub outcomes: Vec<String>,
    pub probabilities: Vec<Rational>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbabilityOperation {
    DistributionConstruction,
    Complement,
    Union,
    Intersection,
    Conditional,
    Independence,
    TotalProbability,
    Bayes,
    Expectation,
    Variance,
    StochasticMatrixCandidate,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbabilityStatus {
    Complete,
    Missing,
    Ambiguous,
    InvalidProbability,
    ZeroConditioning,
    DimensionMismatch,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProbabilityArtifact {
    Distribution(FiniteDistribution),
    Scalar(Rational),
    Boolean(bool),
    ProbabilityVector(Vec<Rational>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbabilityRequest {
    pub operation: ProbabilityOperation,
    pub domain: String,
    pub outcomes: Vec<String>,
    pub probabilities: Vec<Rational>,
    pub values: Vec<i64>,
    pub event_a: Option<Vec<usize>>,
    pub event_b: Option<Vec<usize>>,
    pub partition: Vec<Vec<usize>>,
    pub conditional_values: Vec<Rational>,
    pub prior_probability: Option<Rational>,
    pub likelihood: Option<Rational>,
    pub evidence: Option<Rational>,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbabilitySource {
    pub source_id: String,
    pub title: String,
    pub section: String,
    pub url: String,
    pub license: String,
    pub retrieved_utc: String,
    pub evidence_span: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbabilityResult {
    pub status: ProbabilityStatus,
    pub artifact: Option<ProbabilityArtifact>,
    pub operation: ProbabilityOperation,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub source: ProbabilitySource,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn source() -> ProbabilitySource {
    ProbabilitySource {
        source_id: "openstax-introductory-statistics-2e:probability".into(),
        title: "Introductory Statistics 2e".into(),
        section: "finite probability and discrete random variables".into(),
        url: "https://openstax.org/details/books/introductory-statistics-2e".into(),
        license: "CC BY 4.0; OpenStax attribution required".into(),
        retrieved_utc: "2026-08-05".into(),
        evidence_span: "finite sample spaces, conditional probability, and exact expectations"
            .into(),
    }
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("probability serializes"))
    )
}

fn replay_payload(result: &ProbabilityResult) -> impl Serialize + '_ {
    (
        &result.status,
        &result.artifact,
        result.operation,
        &result.assumptions,
        &result.reasons,
        &result.source,
        &result.provenance,
    )
}

fn compute_replay_hash(result: &ProbabilityResult) -> String {
    digest(&replay_payload(result))
}

fn sum(values: &[Rational]) -> Option<Rational> {
    values
        .iter()
        .try_fold(Rational::zero(), |total, value| total.add(value))
}

fn distribution(request: &ProbabilityRequest) -> Result<FiniteDistribution, ProbabilityStatus> {
    if request.outcomes.is_empty() && request.probabilities.is_empty() {
        return Err(ProbabilityStatus::Missing);
    }
    if request.outcomes.len() != request.probabilities.len() || request.outcomes.is_empty() {
        return Err(ProbabilityStatus::DimensionMismatch);
    }
    if request
        .probabilities
        .iter()
        .any(|probability| !probability.nonnegative())
    {
        return Err(ProbabilityStatus::InvalidProbability);
    }
    if sum(&request.probabilities) != Some(Rational::one()) {
        return Err(ProbabilityStatus::InvalidProbability);
    }
    Ok(FiniteDistribution {
        outcomes: request.outcomes.clone(),
        probabilities: request.probabilities.clone(),
    })
}

fn event_probability(
    probabilities: &[Rational],
    event: &[usize],
) -> Result<Rational, ProbabilityStatus> {
    if event.iter().any(|index| *index >= probabilities.len()) {
        return Err(ProbabilityStatus::DimensionMismatch);
    }
    let mut unique = event.to_vec();
    unique.sort_unstable();
    unique.dedup();
    sum(&unique
        .iter()
        .map(|index| probabilities[*index].clone())
        .collect::<Vec<_>>())
    .ok_or(ProbabilityStatus::InvalidProbability)
}

fn result(
    request: &ProbabilityRequest,
    status: ProbabilityStatus,
    artifact: Option<ProbabilityArtifact>,
    assumptions: Vec<String>,
    reasons: Vec<String>,
) -> ProbabilityResult {
    let mut result = ProbabilityResult {
        status,
        artifact,
        operation: request.operation,
        assumptions,
        reasons,
        source: source(),
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    result.replay_hash = compute_replay_hash(&result);
    result
}

pub fn evaluate_probability(request: &ProbabilityRequest) -> ProbabilityResult {
    if request.domain != "finite_exact_probability" {
        return result(
            request,
            ProbabilityStatus::Unsupported,
            None,
            vec![],
            vec!["domain is outside finite exact probability boundary".into()],
        );
    }
    if let Some(ambiguity) = &request.ambiguity {
        return result(
            request,
            ProbabilityStatus::Ambiguous,
            None,
            vec![],
            vec![ambiguity.clone()],
        );
    }
    if request.operation == ProbabilityOperation::StochasticMatrixCandidate {
        return result(
            request,
            ProbabilityStatus::Unsupported,
            None,
            vec![],
            vec!["stochastic-matrix execution is a later curriculum item".into()],
        );
    }
    if request.operation == ProbabilityOperation::Bayes {
        let (Some(prior), Some(likelihood), Some(evidence)) = (
            &request.prior_probability,
            &request.likelihood,
            &request.evidence,
        ) else {
            return result(
                request,
                ProbabilityStatus::Missing,
                None,
                vec![],
                vec!["Bayes requires prior, likelihood, and evidence".into()],
            );
        };
        if ![prior, likelihood, evidence]
            .iter()
            .all(|value| value.in_unit_interval())
        {
            return result(
                request,
                ProbabilityStatus::InvalidProbability,
                None,
                vec![],
                vec!["Bayes inputs must be probabilities in [0,1]".into()],
            );
        }
        if !evidence.positive() {
            return result(
                request,
                ProbabilityStatus::ZeroConditioning,
                None,
                vec![],
                vec!["Bayes evidence has zero probability".into()],
            );
        }
        let Some(value) = prior.mul(likelihood).and_then(|value| value.div(evidence)) else {
            return result(
                request,
                ProbabilityStatus::InvalidProbability,
                None,
                vec![],
                vec!["rational Bayes computation failed".into()],
            );
        };
        return result(
            request,
            ProbabilityStatus::Complete,
            Some(ProbabilityArtifact::Scalar(value)),
            vec!["finite exact Bayes inputs".into()],
            vec![],
        );
    }
    let distribution = match distribution(request) {
        Ok(value) => value,
        Err(status) => {
            return result(
                request,
                status,
                None,
                vec![],
                vec!["finite distribution is missing, malformed, or not normalized".into()],
            )
        }
    };
    let probabilities = &distribution.probabilities;
    let assumptions = vec![
        "finite sample space".into(),
        "exact rational probabilities".into(),
    ];
    let (status, artifact, reasons) = match request.operation {
        ProbabilityOperation::DistributionConstruction => (
            ProbabilityStatus::Complete,
            Some(ProbabilityArtifact::Distribution(distribution.clone())),
            Vec::new(),
        ),
        ProbabilityOperation::Complement => match &request.event_a {
            Some(event) => match event_probability(probabilities, event).and_then(|value| {
                Rational::one()
                    .sub(&value)
                    .ok_or(ProbabilityStatus::InvalidProbability)
            }) {
                Ok(value) => (
                    ProbabilityStatus::Complete,
                    Some(ProbabilityArtifact::Scalar(value)),
                    Vec::new(),
                ),
                Err(status) => (status, None, vec!["event is invalid".into()]),
            },
            None => (
                ProbabilityStatus::Missing,
                None,
                vec!["complement requires one event".into()],
            ),
        },
        ProbabilityOperation::Union
        | ProbabilityOperation::Intersection
        | ProbabilityOperation::Conditional
        | ProbabilityOperation::Independence => {
            let (Some(event_a), Some(event_b)) = (&request.event_a, &request.event_b) else {
                return result(
                    request,
                    ProbabilityStatus::Missing,
                    None,
                    assumptions,
                    vec!["operation requires two explicit events".into()],
                );
            };
            let intersection = event_a
                .iter()
                .copied()
                .filter(|index| event_b.contains(index))
                .collect::<Vec<_>>();
            let mut union = event_a.clone();
            union.extend(event_b.iter().copied());
            let p_a = event_probability(probabilities, event_a);
            let p_b = event_probability(probabilities, event_b);
            let p_intersection = event_probability(probabilities, &intersection);
            let p_union = event_probability(probabilities, &union);
            match (p_a, p_b, p_intersection, p_union) {
                (Ok(a), Ok(b), Ok(intersection), Ok(_union))
                    if request.operation == ProbabilityOperation::Union =>
                {
                    let value = a.add(&b).and_then(|value| value.sub(&intersection));
                    (
                        ProbabilityStatus::Complete,
                        value.map(ProbabilityArtifact::Scalar),
                        Vec::new(),
                    )
                }
                (Ok(_), Ok(_), Ok(intersection), Ok(_union))
                    if request.operation == ProbabilityOperation::Intersection =>
                {
                    (
                        ProbabilityStatus::Complete,
                        Some(ProbabilityArtifact::Scalar(intersection)),
                        Vec::new(),
                    )
                }
                (Ok(_), Ok(b), Ok(intersection), Ok(_union))
                    if request.operation == ProbabilityOperation::Conditional =>
                {
                    if !b.positive() {
                        (
                            ProbabilityStatus::ZeroConditioning,
                            None,
                            vec!["conditioning event has zero probability".into()],
                        )
                    } else {
                        (
                            ProbabilityStatus::Complete,
                            intersection.div(&b).map(ProbabilityArtifact::Scalar),
                            Vec::new(),
                        )
                    }
                }
                (Ok(a), Ok(b), Ok(intersection), Ok(_union)) => (
                    ProbabilityStatus::Complete,
                    Some(ProbabilityArtifact::Boolean(
                        intersection == a.mul(&b).unwrap(),
                    )),
                    Vec::new(),
                ),
                _ => (
                    ProbabilityStatus::DimensionMismatch,
                    None,
                    vec!["event index is outside the finite sample space".into()],
                ),
            }
        }
        ProbabilityOperation::TotalProbability => {
            if request.partition.is_empty()
                || request.partition.len() != request.conditional_values.len()
                || request.event_a.is_none()
            {
                (
                    ProbabilityStatus::Missing,
                    None,
                    vec!["total probability requires a target and partition".into()],
                )
            } else {
                let mut covered: Vec<usize> = Vec::new();
                let mut total = Rational::zero();
                let target = request.event_a.as_ref().unwrap();
                let mut valid = event_probability(probabilities, target).is_ok();
                for (part, conditional) in request.partition.iter().zip(&request.conditional_values)
                {
                    if part.is_empty() || !conditional.in_unit_interval() {
                        valid = false;
                        break;
                    }
                    let weight = match event_probability(probabilities, part) {
                        Ok(value) => value,
                        Err(_) => {
                            valid = false;
                            break;
                        }
                    };
                    covered.extend(part);
                    total = match weight.mul(conditional).and_then(|value| total.add(&value)) {
                        Some(value) => value,
                        None => {
                            valid = false;
                            break;
                        }
                    };
                }
                covered.sort_unstable();
                covered.dedup();
                let all: Vec<usize> = (0..probabilities.len()).collect();
                let partition_size: usize = request.partition.iter().map(Vec::len).sum();
                if covered != all || partition_size != covered.len() {
                    valid = false;
                }
                if !valid {
                    (ProbabilityStatus::Ambiguous, None, vec!["partition must cover each outcome exactly once with explicit conditionals".into()])
                } else {
                    (
                        ProbabilityStatus::Complete,
                        Some(ProbabilityArtifact::Scalar(total)),
                        Vec::new(),
                    )
                }
            }
        }
        ProbabilityOperation::Expectation | ProbabilityOperation::Variance => {
            if request.values.len() != probabilities.len() {
                (
                    ProbabilityStatus::DimensionMismatch,
                    None,
                    vec!["values and probabilities have different dimensions".into()],
                )
            } else {
                let mean = request
                    .values
                    .iter()
                    .zip(probabilities)
                    .map(|(value, probability)| {
                        probability.mul(&Rational::new(*value as i128, 1).unwrap())
                    })
                    .collect::<Option<Vec<_>>>()
                    .and_then(|terms| sum(&terms));
                let Some(mean) = mean else {
                    return result(
                        request,
                        ProbabilityStatus::InvalidProbability,
                        None,
                        assumptions,
                        vec!["expectation rational computation failed".into()],
                    );
                };
                if request.operation == ProbabilityOperation::Expectation {
                    (
                        ProbabilityStatus::Complete,
                        Some(ProbabilityArtifact::Scalar(mean)),
                        Vec::new(),
                    )
                } else {
                    let squares = request
                        .values
                        .iter()
                        .zip(probabilities)
                        .map(|(value, probability)| {
                            probability.mul(
                                &Rational::new((*value as i128) * (*value as i128), 1).unwrap(),
                            )
                        })
                        .collect::<Option<Vec<_>>>()
                        .and_then(|terms| sum(&terms));
                    let variance = squares.and_then(|second| {
                        mean.mul(&mean)
                            .and_then(|square_mean| second.sub(&square_mean))
                    });
                    (
                        ProbabilityStatus::Complete,
                        variance.map(ProbabilityArtifact::Scalar),
                        Vec::new(),
                    )
                }
            }
        }
        ProbabilityOperation::Bayes | ProbabilityOperation::StochasticMatrixCandidate => {
            unreachable!()
        }
    };
    result(request, status, artifact, assumptions, reasons)
}

impl ProbabilityResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == compute_replay_hash(self)
            && !self.provenance.is_empty()
            && self
                .source
                .source_id
                .starts_with("openstax-introductory-statistics-2e:")
            && (self.status != ProbabilityStatus::Complete || self.artifact.is_some())
    }
}

/// Bridge only degenerate probability vectors into the current integer-vector
/// linear-algebra pack. Non-integral rational vectors remain unconsumed.
pub fn probability_vector_to_linear_algebra(
    result: &ProbabilityResult,
) -> Option<LinearAlgebraRequest> {
    let ProbabilityArtifact::Distribution(distribution) = result.artifact.as_ref()? else {
        return None;
    };
    if distribution
        .probabilities
        .iter()
        .any(|probability| probability.denominator != 1 || ![0, 1].contains(&probability.numerator))
    {
        return None;
    }
    Some(LinearAlgebraRequest {
        operation: LinearAlgebraOperation::VectorConstruction,
        matrix: None,
        vector_a: Some(
            distribution
                .probabilities
                .iter()
                .map(|probability| probability.numerator as i64)
                .collect(),
        ),
        vector_b: None,
        domain: "finite_exact_integer".into(),
        requested_output: "probability_vector".into(),
        provenance: result.provenance.clone(),
    })
}

pub fn probability_vector_artifact(result: &ProbabilityResult) -> Option<LinearAlgebraArtifact> {
    probability_vector_to_linear_algebra(result)
        .and_then(|request| evaluate_vector_request(&request))
}

fn evaluate_vector_request(request: &LinearAlgebraRequest) -> Option<LinearAlgebraArtifact> {
    let vector = request.vector_a.as_ref()?;
    Some(LinearAlgebraArtifact::Vector(vector.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rational(numerator: i128, denominator: i128) -> Rational {
        Rational::new(numerator, denominator).unwrap()
    }
    fn base(operation: ProbabilityOperation) -> ProbabilityRequest {
        ProbabilityRequest {
            operation,
            domain: "finite_exact_probability".into(),
            outcomes: vec!["a".into(), "b".into()],
            probabilities: vec![rational(1, 4), rational(3, 4)],
            values: vec![1, 3],
            event_a: Some(vec![0]),
            event_b: Some(vec![0, 1]),
            partition: Vec::new(),
            conditional_values: Vec::new(),
            prior_probability: None,
            likelihood: None,
            evidence: None,
            ambiguity: None,
            provenance: vec!["test".into()],
        }
    }
    #[test]
    fn exact_conditional_expectation_and_replay() {
        let conditional = evaluate_probability(&base(ProbabilityOperation::Conditional));
        assert_eq!(
            conditional.artifact,
            Some(ProbabilityArtifact::Scalar(rational(1, 4)))
        );
        assert!(conditional.replay_verified());
        let expectation = evaluate_probability(&base(ProbabilityOperation::Expectation));
        assert_eq!(
            expectation.artifact,
            Some(ProbabilityArtifact::Scalar(rational(5, 2)))
        );
    }
    #[test]
    fn invalid_normalization_and_zero_conditioning_refuse() {
        let mut request = base(ProbabilityOperation::DistributionConstruction);
        request.probabilities = vec![rational(1, 2), rational(1, 2)];
        request.outcomes = vec!["a".into(), "b".into(), "c".into()];
        assert_eq!(
            evaluate_probability(&request).status,
            ProbabilityStatus::DimensionMismatch
        );
        let mut zero = base(ProbabilityOperation::Conditional);
        zero.event_b = Some(Vec::new());
        assert_eq!(
            evaluate_probability(&zero).status,
            ProbabilityStatus::ZeroConditioning
        );
    }
    #[test]
    fn rational_probability_vector_bridge_is_narrow() {
        let mut request = base(ProbabilityOperation::DistributionConstruction);
        request.probabilities = vec![Rational::one(), Rational::zero()];
        let result = evaluate_probability(&request);
        assert!(probability_vector_artifact(&result).is_some());
        let fractional =
            evaluate_probability(&base(ProbabilityOperation::DistributionConstruction));
        assert!(probability_vector_to_linear_algebra(&fractional).is_none());
    }
    #[test]
    fn union_and_partition_require_explicit_event_structure() {
        let mut union = base(ProbabilityOperation::Union);
        union.event_b = Some(vec![1]);
        assert_eq!(
            union
                .event_a
                .as_ref()
                .and_then(|_| evaluate_probability(&union).artifact),
            Some(ProbabilityArtifact::Scalar(Rational::one()))
        );

        let mut total = base(ProbabilityOperation::TotalProbability);
        total.partition = vec![vec![0], vec![1]];
        total.conditional_values = vec![rational(1, 2), rational(0, 1)];
        assert_eq!(
            evaluate_probability(&total).artifact,
            Some(ProbabilityArtifact::Scalar(rational(1, 8)))
        );
        total.partition = vec![vec![0], vec![0]];
        assert_eq!(
            evaluate_probability(&total).status,
            ProbabilityStatus::Ambiguous
        );
    }
}
