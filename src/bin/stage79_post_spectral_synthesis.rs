//! Stage 79: expanded cross-domain synthesis after the spectral curriculum.
//!
//! The corpus varies exact spectral inputs and requires explicit lowering into
//! polynomial, number-theory, or foundational linear-algebra artifacts.  It
//! is shadow-only and never changes the live curriculum or router.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::linear_algebra_pack::{
    evaluate_linear_algebra, LinearAlgebraArtifact, LinearAlgebraOperation, LinearAlgebraRequest,
    LinearAlgebraStatus,
};
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryArtifact, NumberTheoryOperation, NumberTheoryRequest,
    NumberTheoryStatus,
};
use the_machine::polynomial_pack::{
    evaluate_polynomial, Polynomial, PolynomialArtifact, PolynomialOperation, PolynomialRequest,
    PolynomialStatus,
};
use the_machine::spectral_linear_algebra_pack::{
    evaluate_spectral, SpectralArtifact, SpectralOperation, SpectralRequest, SpectralStatus,
};

const REPORT_JSON: &str = "docs/stage79_post_spectral_synthesis.json";
const REPORT_MD: &str = "docs/stage79_post_spectral_synthesis.md";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Route {
    CharacteristicToPolynomialToGcd,
    EigenvalueToGcd,
    PowerToLinearAlgebra,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    expected: Expected,
    route: Route,
    matrix: Vec<Vec<i64>>,
    point_or_power: i64,
    refusal_kind: u8,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    route: Route,
    exact: bool,
    spectral_replay: bool,
    downstream_artifacts: usize,
    downstream_replays: usize,
    downstream_tamper_rejections: usize,
    false_authorization: bool,
}

#[derive(Debug, Serialize)]
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
    downstream_artifacts: usize,
    downstream_replays: usize,
    downstream_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_counts: std::collections::BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn spectral_request(
    operation: SpectralOperation,
    matrix: Vec<Vec<i64>>,
    point_or_power: i64,
    route: Route,
) -> SpectralRequest {
    SpectralRequest {
        operation,
        matrix: Some(matrix),
        eigenvalue: None,
        power: (operation == SpectralOperation::MatrixPower).then_some(point_or_power as u32),
        domain: "bounded_exact_spectral_linear_algebra".into(),
        ambiguity: (route == Route::Ambiguous)
            .then(|| "spectral operation or convention is unresolved".into()),
        provenance: vec!["stage79-post-spectral-synthesis".into()],
    }
}

fn number_request(a: i64, b: i64) -> NumberTheoryRequest {
    NumberTheoryRequest {
        operation: NumberTheoryOperation::GcdBezout,
        a: Some(a),
        b: Some(b),
        c: None,
        modulus: None,
        second_modulus: None,
        domain: "bounded_exact_elementary_number_theory".into(),
        ambiguity: None,
        provenance: vec!["stage79-spectral-lowered-number".into()],
    }
}

fn corpus() -> Vec<Case> {
    let mut cases = Vec::with_capacity(1000);
    for index in 0..300 {
        cases.push(Case {
            id: format!("charpoly-{index:03}"),
            expected: Expected::Supported,
            route: Route::CharacteristicToPolynomialToGcd,
            matrix: vec![vec![2, 1], vec![1, 2]],
            point_or_power: (index % 7) as i64,
            refusal_kind: 0,
        });
    }
    for index in 0..150 {
        cases.push(Case {
            id: format!("eigen-{index:03}"),
            expected: Expected::Supported,
            route: Route::EigenvalueToGcd,
            matrix: vec![vec![2 + (index % 2) as i64, 0], vec![0, 5]],
            point_or_power: 0,
            refusal_kind: 0,
        });
    }
    for index in 0..150 {
        cases.push(Case {
            id: format!("power-{index:03}"),
            expected: Expected::Supported,
            route: Route::PowerToLinearAlgebra,
            matrix: vec![vec![2, 1], vec![1, 2]],
            point_or_power: 1 + (index % 8) as i64,
            refusal_kind: 0,
        });
    }
    for index in 0..200 {
        cases.push(Case {
            id: format!("ambiguous-{index:03}"),
            expected: Expected::Ambiguous,
            route: Route::Ambiguous,
            matrix: vec![vec![2, 1], vec![1, 2]],
            point_or_power: 0,
            refusal_kind: 0,
        });
    }
    for index in 0..200 {
        let (matrix, point_or_power) = match index % 4 {
            0 => (vec![vec![0, 1], vec![-1, 0]], 0),
            1 => (vec![vec![2, 1], vec![1, 2]], 9),
            2 => (vec![vec![2, 0], vec![0, 5]], 0),
            _ => (vec![vec![2, 1], vec![1, 2]], 0),
        };
        cases.push(Case {
            id: format!("refused-{index:03}"),
            expected: Expected::Refused,
            route: Route::Refused,
            matrix,
            point_or_power,
            refusal_kind: (index % 4) as u8,
        });
    }
    cases
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases = corpus();
    assert_eq!(cases.len(), 1000);
    let corpus_sha256 = digest(&cases);
    let mut receipts = Vec::with_capacity(cases.len());
    for case in cases {
        let operation = match case.route {
            Route::CharacteristicToPolynomialToGcd => SpectralOperation::CharacteristicPolynomial,
            Route::EigenvalueToGcd => SpectralOperation::IntegerEigenvalues,
            Route::PowerToLinearAlgebra => SpectralOperation::MatrixPower,
            Route::Ambiguous => SpectralOperation::IntegerEigenvalues,
            Route::Refused => match case.refusal_kind {
                0 => SpectralOperation::IntegerEigenvalues,
                1 => SpectralOperation::MatrixPower,
                _ => SpectralOperation::CharacteristicPolynomial,
            },
        };
        let mut request = spectral_request(operation, case.matrix, case.point_or_power, case.route);
        if case.route == Route::Refused && case.refusal_kind >= 2 {
            request.domain = "functional_analysis".into();
        }
        let spectral = evaluate_spectral(&request);
        let mut downstream_artifacts = 0;
        let mut downstream_replays = 0;
        let mut downstream_tamper_rejections = 0;
        let mut route_complete = false;
        match case.route {
            Route::CharacteristicToPolynomialToGcd => {
                if let Some(SpectralArtifact::CharacteristicPolynomial(coefficients)) =
                    spectral.artifact.as_ref()
                {
                    let polynomial = evaluate_polynomial(&PolynomialRequest {
                        operation: PolynomialOperation::Evaluate,
                        left: Some(Polynomial {
                            coefficients: coefficients
                                .iter()
                                .map(|v| v.rem_euclid(7) as u64)
                                .collect(),
                            modulus: 7,
                        }),
                        right: None,
                        point: Some(case.point_or_power as u64),
                        domain: "bounded_exact_prime_field_polynomial".into(),
                        ambiguity: None,
                        provenance: vec!["stage79-characteristic-lowering".into()],
                    });
                    let residue = match polynomial.artifact {
                        Some(PolynomialArtifact::Value(value)) => value as i64,
                        _ => -1,
                    };
                    let arithmetic = evaluate_number_theory(&number_request(residue, 7));
                    route_complete = spectral.status == SpectralStatus::Complete
                        && polynomial.status == PolynomialStatus::Complete
                        && arithmetic.status == NumberTheoryStatus::Complete
                        && matches!(
                            arithmetic.artifact,
                            Some(NumberTheoryArtifact::GcdBezout { .. })
                        );
                    for result in [polynomial.replay_verified(), arithmetic.replay_verified()] {
                        downstream_artifacts += 1;
                        downstream_replays += usize::from(result);
                    }
                    let mut p = polynomial;
                    p.replay_hash.push('x');
                    let mut a = arithmetic;
                    a.replay_hash.push('x');
                    downstream_tamper_rejections +=
                        usize::from(!p.replay_verified()) + usize::from(!a.replay_verified());
                }
            }
            Route::EigenvalueToGcd => {
                if let Some(SpectralArtifact::Eigenvalues(values)) = spectral.artifact.as_ref() {
                    let arithmetic = evaluate_number_theory(&number_request(values[0], 7));
                    route_complete = spectral.status == SpectralStatus::Complete
                        && arithmetic.status == NumberTheoryStatus::Complete;
                    downstream_artifacts = 1;
                    downstream_replays = usize::from(arithmetic.replay_verified());
                    let mut a = arithmetic;
                    a.replay_hash.push('x');
                    downstream_tamper_rejections = usize::from(!a.replay_verified());
                }
            }
            Route::PowerToLinearAlgebra => {
                if let Some(SpectralArtifact::Matrix(matrix)) = spectral.artifact.as_ref() {
                    let matrix = matrix
                        .iter()
                        .map(|row| row.iter().map(|v| *v as i64).collect())
                        .collect::<Vec<Vec<i64>>>();
                    let linear = evaluate_linear_algebra(&LinearAlgebraRequest {
                        operation: LinearAlgebraOperation::MatrixConstruction,
                        matrix: Some(matrix.clone()),
                        vector_a: None,
                        vector_b: None,
                        domain: "finite_exact_integer".into(),
                        requested_output: "matrix".into(),
                        provenance: vec!["stage79-power-lowering".into()],
                    });
                    route_complete = spectral.status == SpectralStatus::Complete
                        && linear.status == LinearAlgebraStatus::Complete
                        && linear.artifact == Some(LinearAlgebraArtifact::Matrix(matrix));
                    downstream_artifacts = 1;
                    downstream_replays = usize::from(linear.replay_verified());
                    let mut l = linear;
                    l.replay_hash.push('x');
                    downstream_tamper_rejections = usize::from(!l.replay_verified());
                }
            }
            Route::Ambiguous | Route::Refused => {}
        }
        let actual = if route_complete {
            Expected::Supported
        } else if spectral.status == SpectralStatus::Ambiguous {
            Expected::Ambiguous
        } else {
            Expected::Refused
        };
        let exact = actual == case.expected;
        let mut spectral_tampered = spectral.clone();
        spectral_tampered.replay_hash.push('x');
        receipts.push(Receipt {
            id: case.id,
            expected: case.expected,
            route: case.route,
            exact,
            spectral_replay: spectral.replay_verified() && !spectral_tampered.replay_verified(),
            downstream_artifacts,
            downstream_replays,
            downstream_tamper_rejections,
            false_authorization: case.expected != Expected::Supported
                && actual == Expected::Supported,
        });
    }
    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|r| r.expected == Expected::Ambiguous)
        .count();
    let refused = receipts
        .iter()
        .filter(|r| r.expected == Expected::Refused)
        .count();
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let supported_routes = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && r.exact)
        .count();
    let spectral_replays = receipts.iter().filter(|r| r.spectral_replay).count();
    let downstream_artifacts = receipts.iter().map(|r| r.downstream_artifacts).sum();
    let downstream_replays = receipts.iter().map(|r| r.downstream_replays).sum();
    let downstream_tamper_rejections = receipts
        .iter()
        .map(|r| r.downstream_tamper_rejections)
        .sum();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && !r.exact)
        .count();
    let mut route_counts = std::collections::BTreeMap::new();
    for receipt in &receipts {
        *route_counts
            .entry(format!("{:?}", receipt.route))
            .or_insert(0usize) += 1;
    }
    assert_eq!((supported, ambiguous, refused), (600, 200, 200));
    assert_eq!(exact_decisions, 1000);
    assert_eq!(supported_routes, 600);
    assert_eq!(spectral_replays, 1000);
    assert_eq!(downstream_artifacts, 900);
    assert_eq!(downstream_replays, 900);
    assert_eq!(downstream_tamper_rejections, 900);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage79-post-spectral-synthesis-v1",
        corpus_sha256,
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_routes,
        spectral_replays,
        downstream_artifacts,
        downstream_replays,
        downstream_tamper_rejections,
        false_authorizations,
        false_denials,
        route_counts,
        receipts,
    };
    std::fs::write(REPORT_JSON, serde_json::to_string_pretty(&report)?)?;
    std::fs::write(REPORT_MD, format!("# Stage 79 — expanded post-spectral synthesis\n\n- Cases: 1,000 (600 supported, 200 ambiguous, 200 refused)\n- Exact decisions: {exact_decisions}/1,000\n- Supported routes: {supported_routes}/600\n- Spectral replay/tamper: {spectral_replays}/1,000\n- Downstream artifacts: {downstream_artifacts}\n- Downstream replay/tamper: {downstream_replays}/{downstream_artifacts} and {downstream_tamper_rejections}/{downstream_artifacts}\n- False authorizations / denials: {false_authorizations} / {false_denials}\n\nThe supported routes lower characteristic polynomials through exact prime-field evaluation and a gcd certificate, exact eigenvalues through a gcd certificate, and bounded matrix powers through the foundational matrix artifact. Ambiguous operations, irrational spectra, invalid domains, and over-budget powers remain closed.\n"))?;
    Ok(())
}
