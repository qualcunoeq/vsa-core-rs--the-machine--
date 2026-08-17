//! Bounded exact complex-analysis theorem contracts.
//!
//! This module is deliberately narrower than complex analysis as a subject.
//! It evaluates finite polynomials in `z` over exact rational complex pairs,
//! checks the Cauchy--Riemann equations for affine real-coordinate maps, and
//! returns replayable certificates.  Polar branches, contour integrals,
//! infinite series, approximation, and unrestricted holomorphic classification
//! remain outside the contract.

use crate::probability_pack::Rational;
use crate::source_formula_pack::SourceCitation;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const DOMAIN: &str = "bounded_exact_complex_analysis";
const MAX_DEGREE: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComplexNumber {
    pub real: Rational,
    pub imag: Rational,
}

impl ComplexNumber {
    pub fn new(real: Rational, imag: Rational) -> Self {
        Self { real, imag }
    }

    fn add(&self, other: &Self) -> Option<Self> {
        Some(Self::new(
            self.real.add(&other.real)?,
            self.imag.add(&other.imag)?,
        ))
    }

    fn mul(&self, other: &Self) -> Option<Self> {
        let real = self
            .real
            .mul(&other.real)?
            .sub(&self.imag.mul(&other.imag)?)?;
        let imag = self
            .real
            .mul(&other.imag)?
            .add(&self.imag.mul(&other.real)?)?;
        Some(Self::new(real, imag))
    }

    fn scale(&self, factor: &Rational) -> Option<Self> {
        Some(Self::new(self.real.mul(factor)?, self.imag.mul(factor)?))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComplexAnalysisOperation {
    PolynomialValue,
    PolynomialDerivative,
    CauchyRiemannCheck,
    AffineHolomorphicDerivative,
    PolarConversion,
    ContourIntegral,
    InfiniteSeries,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComplexAnalysisStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidDomain,
    Inconsistent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ComplexAnalysisArtifact {
    Complex(ComplexNumber),
    Polynomial(Vec<ComplexNumber>),
    CauchyRiemannCertificate {
        holds: bool,
        ux_equals_vy: bool,
        uy_equals_negative_vx: bool,
    },
    Boolean(bool),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComplexAnalysisRequest {
    pub operation: ComplexAnalysisOperation,
    pub coefficients: Vec<ComplexNumber>,
    pub point: Option<ComplexNumber>,
    pub ux: Option<Rational>,
    pub uy: Option<Rational>,
    pub vx: Option<Rational>,
    pub vy: Option<Rational>,
    pub domain: String,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComplexAnalysisResult {
    pub status: ComplexAnalysisStatus,
    pub artifact: Option<ComplexAnalysisArtifact>,
    pub operation: ComplexAnalysisOperation,
    pub assumptions: Vec<String>,
    pub source: SourceCitation,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn source() -> SourceCitation {
    SourceCitation {
        source_id: "openstax-precalculus-2e:complex-analysis-boundary".into(),
        title: "OpenStax Precalculus 2e and attributed complex-analysis theorem notes".into(),
        section: "rectangular complex arithmetic, complex polynomials, and Cauchy-Riemann boundary".into(),
        url: "https://openstax.org/details/books/precalculus-2e".into(),
        license: "CC BY 4.0; OpenStax attribution required".into(),
        retrieved_utc: "2026-08-17".into(),
        evidence_span: "A polynomial in z is evaluated and differentiated componentwise; an affine map satisfies the Cauchy-Riemann equations when ux=vy and uy=-vx.".into(),
    }
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("complex analysis serializes"))
    )
}

fn payload(result: &ComplexAnalysisResult) -> impl Serialize + '_ {
    (
        result.status,
        result.artifact.as_ref(),
        result.operation,
        &result.assumptions,
        &result.source,
        &result.reasons,
        &result.provenance,
    )
}

fn result(
    request: &ComplexAnalysisRequest,
    status: ComplexAnalysisStatus,
    artifact: Option<ComplexAnalysisArtifact>,
    assumptions: Vec<String>,
    reasons: Vec<String>,
) -> ComplexAnalysisResult {
    let mut output = ComplexAnalysisResult {
        status,
        artifact,
        operation: request.operation,
        assumptions,
        source: source(),
        reasons,
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    let replay_hash = digest(&(
        output.status,
        output.artifact.clone(),
        output.operation,
        output.assumptions.clone(),
        output.source.clone(),
        output.reasons.clone(),
        output.provenance.clone(),
    ));
    output.replay_hash = replay_hash;
    output
}

fn polynomial_value(
    coefficients: &[ComplexNumber],
    point: &ComplexNumber,
) -> Option<ComplexNumber> {
    let mut value = ComplexNumber::new(Rational::zero(), Rational::zero());
    for coefficient in coefficients.iter().rev() {
        value = value.mul(point)?.add(coefficient)?;
    }
    Some(value)
}

fn derivative_coefficients(coefficients: &[ComplexNumber]) -> Option<Vec<ComplexNumber>> {
    if coefficients.is_empty() {
        return None;
    }
    Some(
        coefficients
            .iter()
            .enumerate()
            .skip(1)
            .map(|(power, coefficient)| coefficient.scale(&Rational::new(power as i128, 1)?))
            .collect::<Option<Vec<_>>>()?,
    )
}

/// Evaluate the bounded exact complex-analysis contract.
pub fn evaluate_complex_analysis(request: &ComplexAnalysisRequest) -> ComplexAnalysisResult {
    if request.domain != DOMAIN {
        return result(
            request,
            ComplexAnalysisStatus::InvalidDomain,
            None,
            Vec::new(),
            vec!["domain is outside bounded exact complex analysis".into()],
        );
    }
    if let Some(ambiguity) = &request.ambiguity {
        return result(
            request,
            ComplexAnalysisStatus::Ambiguous,
            None,
            Vec::new(),
            vec![ambiguity.clone()],
        );
    }
    if matches!(
        request.operation,
        ComplexAnalysisOperation::PolarConversion
            | ComplexAnalysisOperation::ContourIntegral
            | ComplexAnalysisOperation::InfiniteSeries
    ) {
        return result(
            request,
            ComplexAnalysisStatus::Unsupported,
            None,
            vec!["the operation requires branch, contour, or infinite-series semantics".into()],
            vec!["outside the bounded rectangular theorem contract".into()],
        );
    }
    match request.operation {
        ComplexAnalysisOperation::PolynomialValue => {
            if request.coefficients.is_empty() || request.coefficients.len() > MAX_DEGREE + 1 {
                return result(
                    request,
                    ComplexAnalysisStatus::Unsupported,
                    None,
                    Vec::new(),
                    vec!["polynomial degree must be between zero and four".into()],
                );
            }
            let Some(point) = &request.point else {
                return result(
                    request,
                    ComplexAnalysisStatus::Missing,
                    None,
                    Vec::new(),
                    vec!["evaluation point is required".into()],
                );
            };
            let Some(value) = polynomial_value(&request.coefficients, point) else {
                return result(
                    request,
                    ComplexAnalysisStatus::Inconsistent,
                    None,
                    Vec::new(),
                    vec!["exact rational arithmetic overflowed its bounded representation".into()],
                );
            };
            result(
                request,
                ComplexAnalysisStatus::Complete,
                Some(ComplexAnalysisArtifact::Complex(value)),
                vec!["finite polynomial in z is holomorphic on the complex plane".into()],
                Vec::new(),
            )
        }
        ComplexAnalysisOperation::PolynomialDerivative => {
            if request.coefficients.is_empty() || request.coefficients.len() > MAX_DEGREE + 1 {
                return result(
                    request,
                    ComplexAnalysisStatus::Unsupported,
                    None,
                    Vec::new(),
                    vec!["polynomial degree must be between zero and four".into()],
                );
            }
            let Some(derivative) = derivative_coefficients(&request.coefficients) else {
                return result(
                    request,
                    ComplexAnalysisStatus::Inconsistent,
                    None,
                    Vec::new(),
                    vec!["derivative could not be constructed".into()],
                );
            };
            result(
                request,
                ComplexAnalysisStatus::Complete,
                Some(ComplexAnalysisArtifact::Polynomial(derivative)),
                vec!["termwise derivative is authorized for finite polynomials in z".into()],
                Vec::new(),
            )
        }
        ComplexAnalysisOperation::CauchyRiemannCheck
        | ComplexAnalysisOperation::AffineHolomorphicDerivative => {
            let (Some(ux), Some(uy), Some(vx), Some(vy)) = (
                request.ux.as_ref(),
                request.uy.as_ref(),
                request.vx.as_ref(),
                request.vy.as_ref(),
            ) else {
                return result(
                    request,
                    ComplexAnalysisStatus::Missing,
                    None,
                    Vec::new(),
                    vec!["all four affine partial derivatives are required".into()],
                );
            };
            let ux_equals_vy = ux == vy;
            let uy_equals_negative_vx =
                uy == &Rational::new(-vx.numerator, vx.denominator).expect("nonzero denominator");
            let holds = ux_equals_vy && uy_equals_negative_vx;
            let certificate = ComplexAnalysisArtifact::CauchyRiemannCertificate {
                holds,
                ux_equals_vy,
                uy_equals_negative_vx,
            };
            if request.operation == ComplexAnalysisOperation::CauchyRiemannCheck {
                return result(request, ComplexAnalysisStatus::Complete, Some(certificate), vec!["Cauchy-Riemann equations were checked at the explicitly declared affine derivatives".into()], Vec::new());
            }
            if !holds {
                return result(request, ComplexAnalysisStatus::Inconsistent, Some(certificate), vec!["Cauchy-Riemann equations fail; an affine map is not complex differentiable".into()], vec!["derivative authorization is refused".into()]);
            }
            result(
                request,
                ComplexAnalysisStatus::Complete,
                Some(ComplexAnalysisArtifact::Complex(ComplexNumber::new(
                    ux.clone(),
                    vx.clone(),
                ))),
                vec!["Cauchy-Riemann equations authorize the affine complex derivative".into()],
                Vec::new(),
            )
        }
        ComplexAnalysisOperation::PolarConversion
        | ComplexAnalysisOperation::ContourIntegral
        | ComplexAnalysisOperation::InfiniteSeries => unreachable!(),
    }
}

pub fn replay_verified(result: &ComplexAnalysisResult) -> bool {
    result.replay_hash == digest(&payload(result)) && !result.provenance.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(numerator: i128, denominator: i128) -> Rational {
        Rational::new(numerator, denominator).unwrap()
    }

    fn z(real: i128, imag: i128) -> ComplexNumber {
        ComplexNumber::new(q(real, 1), q(imag, 1))
    }

    fn request(operation: ComplexAnalysisOperation) -> ComplexAnalysisRequest {
        ComplexAnalysisRequest {
            operation,
            coefficients: vec![z(1, 0), z(2, 1), z(1, 0)],
            point: Some(z(1, 1)),
            ux: Some(q(2, 1)),
            uy: Some(q(-1, 1)),
            vx: Some(q(1, 1)),
            vy: Some(q(2, 1)),
            domain: DOMAIN.into(),
            ambiguity: None,
            provenance: vec!["bounded-complex-analysis-test".into()],
        }
    }

    #[test]
    fn polynomial_value_and_derivative_replay() {
        for operation in [
            ComplexAnalysisOperation::PolynomialValue,
            ComplexAnalysisOperation::PolynomialDerivative,
        ] {
            let result = evaluate_complex_analysis(&request(operation));
            assert_eq!(result.status, ComplexAnalysisStatus::Complete);
            assert!(replay_verified(&result));
        }
    }

    #[test]
    fn cauchy_riemann_certificate_is_explicit() {
        let result =
            evaluate_complex_analysis(&request(ComplexAnalysisOperation::CauchyRiemannCheck));
        assert_eq!(result.status, ComplexAnalysisStatus::Complete);
        assert!(replay_verified(&result));
        assert!(matches!(
            result.artifact,
            Some(ComplexAnalysisArtifact::CauchyRiemannCertificate { holds: true, .. })
        ));
    }

    #[test]
    fn nonholomorphic_affine_derivative_is_rejected() {
        let mut request = request(ComplexAnalysisOperation::AffineHolomorphicDerivative);
        request.vy = Some(q(3, 1));
        let result = evaluate_complex_analysis(&request);
        assert_eq!(result.status, ComplexAnalysisStatus::Inconsistent);
        assert!(replay_verified(&result));
    }

    #[test]
    fn polar_and_infinite_operations_remain_unsupported() {
        for operation in [
            ComplexAnalysisOperation::PolarConversion,
            ComplexAnalysisOperation::ContourIntegral,
            ComplexAnalysisOperation::InfiniteSeries,
        ] {
            let result = evaluate_complex_analysis(&request(operation));
            assert_eq!(result.status, ComplexAnalysisStatus::Unsupported);
            assert!(replay_verified(&result));
        }
    }
}
