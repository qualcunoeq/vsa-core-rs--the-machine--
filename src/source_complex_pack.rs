//! Source-derived bounded complex arithmetic.
//!
//! The operation semantics live in an attributed source document as paired
//! real/imaginary formula records.  This module only validates the pairing,
//! supplies typed complex inputs, and delegates expression evaluation to the
//! generic source-formula interpreter.  It deliberately stops before polar,
//! analytic, or approximate complex analysis.

use crate::probability_pack::Rational;
use crate::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaRecord, FormulaRequest,
    FormulaStatus, SourceCitation,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const DOMAIN: &str = "source_derived_complex_arithmetic";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComplexOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Conjugate,
    NormSquared,
    PolarConversion,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComplexStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidDomain,
    Inconsistent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ComplexArtifact {
    Pair { real: Rational, imag: Rational },
    Scalar(Rational),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComplexRequest {
    pub operation: ComplexOperation,
    pub a: Option<Rational>,
    pub b: Option<Rational>,
    pub c: Option<Rational>,
    pub d: Option<Rational>,
    pub domain: String,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComplexResult {
    pub status: ComplexStatus,
    pub artifact: Option<ComplexArtifact>,
    pub operation: ComplexOperation,
    pub assumptions: Vec<String>,
    pub sources: Vec<SourceCitation>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("complex result serializes"))
    )
}

fn payload(result: &ComplexResult) -> impl Serialize + '_ {
    (
        result.status,
        result.artifact.as_ref(),
        result.operation,
        &result.assumptions,
        &result.sources,
        &result.reasons,
        &result.provenance,
    )
}

fn result(
    request: &ComplexRequest,
    status: ComplexStatus,
    artifact: Option<ComplexArtifact>,
    assumptions: Vec<String>,
    sources: Vec<SourceCitation>,
    reasons: Vec<String>,
) -> ComplexResult {
    let mut output = ComplexResult {
        status,
        artifact,
        operation: request.operation,
        assumptions,
        sources,
        reasons,
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&output));
    output.replay_hash = replay_hash;
    output
}

fn records() -> Vec<FormulaRecord> {
    extract_formula_records(include_str!(
        "../docs/sources/openstax_complex_arithmetic_source.txt"
    ))
    .expect("source-derived complex document extracts and validates")
}

fn formula_ids(operation: ComplexOperation) -> (&'static str, Option<&'static str>) {
    match operation {
        ComplexOperation::Add => ("complex_add_real", Some("complex_add_imag")),
        ComplexOperation::Subtract => ("complex_subtract_real", Some("complex_subtract_imag")),
        ComplexOperation::Multiply => ("complex_multiply_real", Some("complex_multiply_imag")),
        ComplexOperation::Divide => ("complex_divide_real", Some("complex_divide_imag")),
        ComplexOperation::Conjugate => ("complex_conjugate_real", Some("complex_conjugate_imag")),
        ComplexOperation::NormSquared => ("complex_norm_squared", None),
        ComplexOperation::PolarConversion => ("", None),
    }
}

fn inputs(request: &ComplexRequest) -> Option<BTreeMap<String, Rational>> {
    let mut values = BTreeMap::new();
    match request.operation {
        ComplexOperation::Conjugate | ComplexOperation::NormSquared => {
            values.insert("a".into(), request.a.clone()?);
            values.insert("b".into(), request.b.clone()?);
        }
        ComplexOperation::PolarConversion => return None,
        _ => {
            values.insert("a".into(), request.a.clone()?);
            values.insert("b".into(), request.b.clone()?);
            values.insert("c".into(), request.c.clone()?);
            values.insert("d".into(), request.d.clone()?);
        }
    }
    Some(values)
}

fn evaluate_component(
    id: &str,
    values: &BTreeMap<String, Rational>,
    request: &ComplexRequest,
    catalog: &[FormulaRecord],
) -> crate::source_formula_pack::FormulaResult {
    evaluate_formula_records(
        &FormulaRequest {
            formula: id.into(),
            inputs: values.clone(),
            domain: DOMAIN.into(),
            ambiguity: None,
            provenance: request.provenance.clone(),
        },
        DOMAIN,
        catalog,
    )
}

fn map_status(status: FormulaStatus) -> ComplexStatus {
    match status {
        FormulaStatus::Complete => ComplexStatus::Complete,
        FormulaStatus::Missing => ComplexStatus::Missing,
        FormulaStatus::Ambiguous => ComplexStatus::Ambiguous,
        FormulaStatus::Unsupported => ComplexStatus::Unsupported,
        FormulaStatus::InvalidDomain => ComplexStatus::InvalidDomain,
        FormulaStatus::Inconsistent => ComplexStatus::Inconsistent,
    }
}

/// Evaluate one exact, source-derived complex arithmetic operation.
pub fn evaluate_complex(request: &ComplexRequest) -> ComplexResult {
    if request.domain != DOMAIN {
        return result(
            request,
            ComplexStatus::InvalidDomain,
            None,
            Vec::new(),
            Vec::new(),
            vec!["domain is outside source-derived complex arithmetic".into()],
        );
    }
    if let Some(ambiguity) = &request.ambiguity {
        return result(
            request,
            ComplexStatus::Ambiguous,
            None,
            Vec::new(),
            Vec::new(),
            vec![ambiguity.clone()],
        );
    }
    if request.operation == ComplexOperation::PolarConversion {
        return result(
            request,
            ComplexStatus::Unsupported,
            None,
            vec![
                "polar conversion requires trigonometric and branch semantics outside this pack"
                    .into(),
            ],
            Vec::new(),
            vec!["operation is outside the bounded rectangular arithmetic scope".into()],
        );
    }
    let Some(values) = inputs(request) else {
        return result(
            request,
            ComplexStatus::Missing,
            None,
            Vec::new(),
            Vec::new(),
            vec!["all operation-specific real and imaginary components are required".into()],
        );
    };
    if request.operation == ComplexOperation::Divide
        && request.c.as_ref().is_some_and(|value| value.numerator == 0)
        && request.d.as_ref().is_some_and(|value| value.numerator == 0)
    {
        return result(
            request,
            ComplexStatus::Inconsistent,
            None,
            vec!["the complex divisor must be nonzero".into()],
            Vec::new(),
            vec!["division by zero complex number is rejected".into()],
        );
    }
    let catalog = records();
    let (real_id, imag_id) = formula_ids(request.operation);
    let real = evaluate_component(real_id, &values, request, &catalog);
    if real.status != FormulaStatus::Complete {
        return result(
            request,
            map_status(real.status),
            None,
            real.assumptions,
            real.source.into_iter().collect(),
            real.reasons,
        );
    }
    let mut sources = real.source.clone().into_iter().collect::<Vec<_>>();
    let assumptions = real.assumptions.clone();
    let Some(real_value) = real.value else {
        return result(
            request,
            ComplexStatus::Unsupported,
            None,
            assumptions,
            sources,
            vec!["source expression did not produce an exact rational value".into()],
        );
    };
    let Some(imag_id) = imag_id else {
        return result(
            request,
            ComplexStatus::Complete,
            Some(ComplexArtifact::Scalar(real_value)),
            assumptions,
            sources,
            Vec::new(),
        );
    };
    let imag = evaluate_component(imag_id, &values, request, &catalog);
    if imag.status != FormulaStatus::Complete {
        return result(
            request,
            map_status(imag.status),
            None,
            assumptions,
            sources,
            imag.reasons,
        );
    }
    sources.extend(imag.source);
    let Some(imag_value) = imag.value else {
        return result(
            request,
            ComplexStatus::Unsupported,
            None,
            assumptions,
            sources,
            vec!["source expression did not produce an exact rational value".into()],
        );
    };
    result(
        request,
        ComplexStatus::Complete,
        Some(ComplexArtifact::Pair {
            real: real_value,
            imag: imag_value,
        }),
        assumptions,
        sources,
        Vec::new(),
    )
}

impl ComplexResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != ComplexStatus::Complete
                || (self.artifact.is_some() && !self.sources.is_empty()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(numerator: i128, denominator: i128) -> Rational {
        Rational::new(numerator, denominator).unwrap()
    }

    fn request(operation: ComplexOperation) -> ComplexRequest {
        ComplexRequest {
            operation,
            a: Some(q(3, 1)),
            b: Some(q(-4, 1)),
            c: Some(q(2, 1)),
            d: Some(q(5, 1)),
            domain: DOMAIN.into(),
            ambiguity: None,
            provenance: vec!["unit-test".into()],
        }
    }

    #[test]
    fn source_complex_multiplication_is_exact_and_replayable() {
        let output = evaluate_complex(&request(ComplexOperation::Multiply));
        assert_eq!(
            output.artifact,
            Some(ComplexArtifact::Pair {
                real: q(26, 1),
                imag: q(7, 1),
            })
        );
        assert!(output.replay_verified());
    }

    #[test]
    fn zero_divisor_and_polar_conversion_fail_closed() {
        let mut divide = request(ComplexOperation::Divide);
        divide.c = Some(q(0, 1));
        divide.d = Some(q(0, 1));
        assert_eq!(
            evaluate_complex(&divide).status,
            ComplexStatus::Inconsistent
        );
        assert_eq!(
            evaluate_complex(&request(ComplexOperation::PolarConversion)).status,
            ComplexStatus::Unsupported
        );
    }
}
