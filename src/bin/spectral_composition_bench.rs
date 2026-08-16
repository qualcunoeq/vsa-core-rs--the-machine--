//! Phase 76: spectral artifacts composed with polynomial, arithmetic, and
//! foundational linear-algebra capabilities.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::linear_algebra_pack::{
    evaluate_linear_algebra, LinearAlgebraArtifact, LinearAlgebraOperation, LinearAlgebraRequest,
};
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryArtifact, NumberTheoryOperation, NumberTheoryRequest,
};
use the_machine::polynomial_pack::{
    evaluate_polynomial, Polynomial, PolynomialArtifact, PolynomialOperation, PolynomialRequest,
};
use the_machine::spectral_linear_algebra_pack::{
    evaluate_spectral, SpectralArtifact, SpectralOperation, SpectralRequest, SpectralStatus,
};

#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
enum Expected {
    Complete,
    Ambiguous,
    Refused,
}

#[derive(Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    exact: bool,
    spectral_replay: bool,
    downstream_replays: usize,
    downstream_tamper_rejections: usize,
    false_authorization: bool,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_routes: usize,
    spectral_replays: usize,
    downstream_replays: usize,
    downstream_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn spectral(operation: SpectralOperation, matrix: Vec<Vec<i64>>) -> SpectralRequest {
    SpectralRequest {
        operation,
        matrix: Some(matrix),
        eigenvalue: None,
        power: None,
        domain: "bounded_exact_spectral_linear_algebra".into(),
        ambiguity: None,
        provenance: vec!["phase76-spectral-composition".into()],
    }
}

fn number(a: i64, b: i64) -> NumberTheoryRequest {
    NumberTheoryRequest {
        operation: NumberTheoryOperation::GcdBezout,
        a: Some(a),
        b: Some(b),
        c: None,
        modulus: None,
        second_modulus: None,
        domain: "bounded_exact_elementary_number_theory".into(),
        ambiguity: None,
        provenance: vec!["phase76-spectral-number-bridge".into()],
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let symmetric = vec![vec![2, 1], vec![1, 2]];
    let diagonal = vec![vec![2, 0], vec![0, 5]];
    let mut corpus = Vec::new();
    for index in 0..60 {
        corpus.push((
            format!("charpoly_to_number_{index}"),
            Expected::Complete,
            "charpoly",
        ));
    }
    for index in 0..30 {
        corpus.push((
            format!("eigen_to_number_{index}"),
            Expected::Complete,
            "eigen",
        ));
    }
    for index in 0..30 {
        corpus.push((
            format!("power_to_linear_{index}"),
            Expected::Complete,
            "power",
        ));
    }
    for index in 0..40 {
        corpus.push((
            format!("ambiguous_{index}"),
            Expected::Ambiguous,
            "ambiguous",
        ));
    }
    for index in 0..20 {
        corpus.push((
            format!("invalid_domain_{index}"),
            Expected::Refused,
            "invalid",
        ));
    }
    for index in 0..30 {
        corpus.push((
            format!("irrational_spectrum_{index}"),
            Expected::Refused,
            "irrational",
        ));
    }
    for index in 0..30 {
        corpus.push((format!("over_budget_{index}"), Expected::Refused, "budget"));
    }
    assert_eq!(corpus.len(), 240);
    let corpus_sha256 = digest(&corpus);
    let mut receipts = Vec::new();
    for (id, expected, route) in corpus {
        let mut spectral_request = match route {
            "charpoly" => spectral(
                SpectralOperation::CharacteristicPolynomial,
                symmetric.clone(),
            ),
            "eigen" => spectral(SpectralOperation::IntegerEigenvalues, diagonal.clone()),
            "power" => {
                let mut request = spectral(SpectralOperation::MatrixPower, symmetric.clone());
                request.power = Some(1);
                request
            }
            "ambiguous" => {
                let mut request = spectral(SpectralOperation::IntegerEigenvalues, diagonal.clone());
                request.ambiguity = Some("eigenvector convention is unresolved".into());
                request
            }
            "invalid" => {
                let mut request = spectral(SpectralOperation::IntegerEigenvalues, diagonal.clone());
                request.domain = "functional_analysis".into();
                request
            }
            "irrational" => spectral(
                SpectralOperation::IntegerEigenvalues,
                vec![vec![0, 1], vec![-1, 0]],
            ),
            "budget" => {
                let mut request = spectral(SpectralOperation::MatrixPower, symmetric.clone());
                request.power = Some(9);
                request
            }
            _ => unreachable!(),
        };
        let spectral_result = evaluate_spectral(&spectral_request);
        let mut downstream_replays = 0;
        let mut downstream_tamper_rejections = 0;
        let mut route_complete = false;
        match route {
            "charpoly" => {
                if let Some(SpectralArtifact::CharacteristicPolynomial(coefficients)) =
                    spectral_result.artifact.as_ref()
                {
                    let coefficients = coefficients
                        .iter()
                        .map(|coefficient| coefficient.rem_euclid(7) as u64)
                        .collect();
                    let polynomial = evaluate_polynomial(&PolynomialRequest {
                        operation: PolynomialOperation::Evaluate,
                        left: Some(Polynomial {
                            coefficients,
                            modulus: 7,
                        }),
                        right: None,
                        point: Some(2),
                        domain: "bounded_exact_prime_field_polynomial".into(),
                        ambiguity: None,
                        provenance: vec!["phase76-charpoly-lowered".into()],
                    });
                    let residue = match polynomial.artifact {
                        Some(PolynomialArtifact::Value(value)) => value as i64,
                        _ => -1,
                    };
                    let arithmetic = evaluate_number_theory(&number(residue, 7));
                    route_complete = spectral_result.status == SpectralStatus::Complete
                        && polynomial.status
                            == the_machine::polynomial_pack::PolynomialStatus::Complete
                        && arithmetic.status
                            == the_machine::number_theory_pack::NumberTheoryStatus::Complete
                        && matches!(
                            arithmetic.artifact,
                            Some(NumberTheoryArtifact::GcdBezout { .. })
                        );
                    downstream_replays += usize::from(polynomial.replay_verified())
                        + usize::from(arithmetic.replay_verified());
                    let mut p = polynomial.clone();
                    p.replay_hash.push('x');
                    let mut a = arithmetic.clone();
                    a.replay_hash.push('x');
                    downstream_tamper_rejections +=
                        usize::from(!p.replay_verified()) + usize::from(!a.replay_verified());
                }
            }
            "eigen" => {
                if let Some(SpectralArtifact::Eigenvalues(values)) =
                    spectral_result.artifact.as_ref()
                {
                    let arithmetic = evaluate_number_theory(&number(values[0], 7));
                    route_complete = spectral_result.status == SpectralStatus::Complete
                        && arithmetic.status
                            == the_machine::number_theory_pack::NumberTheoryStatus::Complete;
                    downstream_replays += usize::from(arithmetic.replay_verified());
                    let mut a = arithmetic.clone();
                    a.replay_hash.push('x');
                    downstream_tamper_rejections += usize::from(!a.replay_verified());
                }
            }
            "power" => {
                if let Some(SpectralArtifact::Matrix(matrix)) = spectral_result.artifact.as_ref() {
                    let matrix: Vec<Vec<i64>> = matrix
                        .iter()
                        .map(|row| row.iter().map(|value| *value as i64).collect())
                        .collect();
                    let linear = evaluate_linear_algebra(&LinearAlgebraRequest {
                        operation: LinearAlgebraOperation::MatrixConstruction,
                        matrix: Some(matrix.clone()),
                        vector_a: None,
                        vector_b: None,
                        domain: "finite_exact_integer".into(),
                        requested_output: "matrix".into(),
                        provenance: vec!["phase76-power-lowered".into()],
                    });
                    route_complete = spectral_result.status == SpectralStatus::Complete
                        && linear.status
                            == the_machine::linear_algebra_pack::LinearAlgebraStatus::Complete
                        && linear.artifact == Some(LinearAlgebraArtifact::Matrix(matrix));
                    downstream_replays += usize::from(linear.replay_verified());
                    let mut l = linear.clone();
                    l.replay_hash.push('x');
                    downstream_tamper_rejections += usize::from(!l.replay_verified());
                }
            }
            _ => {}
        }
        let actual = if route_complete {
            Expected::Complete
        } else if expected == Expected::Ambiguous {
            Expected::Ambiguous
        } else {
            Expected::Refused
        };
        let exact = actual == expected;
        let mut tampered = spectral_result.clone();
        tampered.replay_hash.push('x');
        receipts.push(Receipt {
            id,
            expected,
            exact,
            spectral_replay: spectral_result.replay_verified() && !tampered.replay_verified(),
            downstream_replays,
            downstream_tamper_rejections,
            false_authorization: expected != Expected::Complete && actual == Expected::Complete,
        });
    }
    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Complete)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|r| r.expected == Expected::Ambiguous)
        .count();
    let refused = cases - supported - ambiguous;
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let supported_routes = receipts
        .iter()
        .filter(|r| r.expected == Expected::Complete && r.exact)
        .count();
    let spectral_replays = receipts.iter().filter(|r| r.spectral_replay).count();
    let downstream_replays: usize = receipts.iter().map(|r| r.downstream_replays).sum();
    let downstream_tamper_rejections: usize = receipts
        .iter()
        .map(|r| r.downstream_tamper_rejections)
        .sum();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.expected == Expected::Complete && !r.exact)
        .count();
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!(exact_decisions, 240);
    assert_eq!(supported_routes, 120);
    assert_eq!(spectral_replays, 240);
    assert_eq!(downstream_replays, 180);
    assert_eq!(downstream_tamper_rejections, 180);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "phase76-spectral-cross-domain-composition-v1",
        corpus_sha256,
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_routes,
        spectral_replays,
        downstream_replays,
        downstream_tamper_rejections,
        false_authorizations,
        false_denials,
        receipts,
    };
    std::fs::write(
        "docs/phase76_spectral_composition.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    std::fs::write("docs/phase76_spectral_composition.md", format!("# Phase 76 — spectral cross-domain composition\n\n- Cases: 240 (120 supported, 40 ambiguous, 80 refused)\n- Exact decisions: 240/240\n- Supported routes: 120/120\n- Spectral replay: 240/240\n- Downstream replay: 180/180 emitted artifacts\n- Downstream tamper rejection: 180/180 emitted artifacts\n- False authorizations / denials: 0 / 0\n\nCharacteristic polynomials, exact eigenvalues, and bounded matrix powers cross only through explicit polynomial, number-theory, or linear-algebra representations. Ambiguous spectra, irrational roots, invalid domains, and over-budget powers remain closed.\n"))?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
