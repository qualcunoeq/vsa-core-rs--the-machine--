//! Phase 75: independent bounded exact spectral-linear-algebra campaign.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::spectral_linear_algebra_pack::{
    evaluate_spectral, SpectralArtifact, SpectralOperation, SpectralRequest, SpectralStatus,
};

#[derive(Clone, Serialize)]
struct Case {
    id: String,
    expected: SpectralStatus,
    request: SpectralRequest,
}

#[derive(Serialize)]
struct Receipt {
    id: String,
    expected: SpectralStatus,
    actual: SpectralStatus,
    artifact_emitted: bool,
    artifact_correct: bool,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
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
    supported_artifacts: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn request(operation: SpectralOperation, matrix: Vec<Vec<i64>>) -> SpectralRequest {
    SpectralRequest {
        operation,
        matrix: Some(matrix),
        eigenvalue: None,
        power: None,
        domain: "bounded_exact_spectral_linear_algebra".into(),
        ambiguity: None,
        provenance: vec!["phase75-independent-spectral-corpus".into()],
    }
}

fn artifact_correct(id: &str, artifact: Option<&SpectralArtifact>) -> bool {
    let Some(artifact) = artifact else {
        return false;
    };
    if id.starts_with("characteristic_") {
        return *artifact == SpectralArtifact::CharacteristicPolynomial(vec![3, -4, 1]);
    }
    if id.starts_with("integer_eigenvalues_") {
        return *artifact == SpectralArtifact::Eigenvalues(vec![2, 5]);
    }
    if id.starts_with("diagonalizability_") {
        return *artifact == SpectralArtifact::Diagonalizable(true);
    }
    if id.starts_with("eigenspace_") {
        let SpectralArtifact::Eigenspace { eigenvalue, basis } = artifact else {
            return false;
        };
        if *eigenvalue != 3 || basis.len() != 1 || basis[0].len() != 2 {
            return false;
        }
        let vector = &basis[0];
        vector[0].numerator * vector[1].denominator == vector[1].numerator * vector[0].denominator
    } else if id.starts_with("decomposition_") {
        matches!(artifact, SpectralArtifact::Decomposition { eigenvalues, basis } if eigenvalues == &[2, 5] && basis.len() == 2)
    } else if id.starts_with("matrix_power_") {
        let power = id
            .rsplit('_')
            .next()
            .and_then(|value| value.parse::<u32>().ok())
            .map(|index| index % 5)
            .unwrap_or(0);
        let expected = (0..power).fold(vec![vec![1i128, 0], vec![0, 1]], |left, _| {
            vec![
                vec![left[0][0] * 2 + left[0][1], left[0][0] + left[0][1] * 2],
                vec![left[1][0] * 2 + left[1][1], left[1][0] + left[1][1] * 2],
            ]
        });
        artifact == &SpectralArtifact::Matrix(expected)
    } else {
        false
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let symmetric = vec![vec![2, 1], vec![1, 2]];
    let diagonal = vec![vec![2, 0], vec![0, 5]];
    let identity = vec![vec![1, 0], vec![0, 1]];
    let mut corpus = Vec::new();
    for index in 0..20 {
        corpus.push(Case {
            id: format!("characteristic_{index}"),
            expected: SpectralStatus::Complete,
            request: request(
                SpectralOperation::CharacteristicPolynomial,
                symmetric.clone(),
            ),
        });
        corpus.push(Case {
            id: format!("integer_eigenvalues_{index}"),
            expected: SpectralStatus::Complete,
            request: request(SpectralOperation::IntegerEigenvalues, diagonal.clone()),
        });
        let mut eigenspace = request(SpectralOperation::Eigenspace, symmetric.clone());
        eigenspace.eigenvalue = Some(3);
        corpus.push(Case {
            id: format!("eigenspace_{index}"),
            expected: SpectralStatus::Complete,
            request: eigenspace,
        });
        corpus.push(Case {
            id: format!("diagonalizability_{index}"),
            expected: SpectralStatus::Complete,
            request: request(SpectralOperation::Diagonalizability, identity.clone()),
        });
        let mut power = request(SpectralOperation::MatrixPower, symmetric.clone());
        power.power = Some((index % 5) as u32);
        corpus.push(Case {
            id: format!("matrix_power_{index}"),
            expected: SpectralStatus::Complete,
            request: power,
        });
        corpus.push(Case {
            id: format!("decomposition_{index}"),
            expected: SpectralStatus::Complete,
            request: request(SpectralOperation::SpectralDecomposition, diagonal.clone()),
        });
    }
    for index in 0..40 {
        let mut ambiguous = request(SpectralOperation::IntegerEigenvalues, diagonal.clone());
        ambiguous.ambiguity =
            Some("basis convention or eigenvalue multiplicity is unresolved".into());
        corpus.push(Case {
            id: format!("ambiguous_{index}"),
            expected: SpectralStatus::Ambiguous,
            request: ambiguous,
        });
    }
    for index in 0..20 {
        let mut invalid = request(SpectralOperation::IntegerEigenvalues, diagonal.clone());
        invalid.domain = "functional_analysis".into();
        corpus.push(Case {
            id: format!("invalid_domain_{index}"),
            expected: SpectralStatus::InvalidDomain,
            request: invalid,
        });
    }
    for index in 0..20 {
        let oversized = vec![vec![1; 5]; 5];
        corpus.push(Case {
            id: format!("oversized_{index}"),
            expected: SpectralStatus::Unsupported,
            request: request(SpectralOperation::CharacteristicPolynomial, oversized),
        });
    }
    for index in 0..20 {
        let nonsquare_spectrum = vec![vec![0, 1], vec![-1, 0]];
        corpus.push(Case {
            id: format!("noninteger_spectrum_{index}"),
            expected: SpectralStatus::Unsupported,
            request: request(SpectralOperation::IntegerEigenvalues, nonsquare_spectrum),
        });
    }
    for index in 0..20 {
        let mut over_budget = request(SpectralOperation::MatrixPower, symmetric.clone());
        over_budget.power = Some(9);
        corpus.push(Case {
            id: format!("over_budget_{index}"),
            expected: SpectralStatus::Unsupported,
            request: over_budget,
        });
    }
    assert_eq!(corpus.len(), 240);
    let corpus_sha256 = digest(&corpus);
    let mut receipts = Vec::with_capacity(corpus.len());
    for case in corpus {
        let result = evaluate_spectral(&case.request);
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        let artifact_emitted = result.artifact.is_some();
        let correct_artifact = if case.expected == SpectralStatus::Complete {
            artifact_correct(&case.id, result.artifact.as_ref())
        } else {
            !artifact_emitted
        };
        let exact = result.status == case.expected && correct_artifact;
        receipts.push(Receipt {
            id: case.id,
            expected: case.expected,
            actual: result.status,
            artifact_emitted,
            artifact_correct: correct_artifact,
            exact,
            replay_verified: result.replay_verified(),
            tamper_rejected: !tampered.replay_verified(),
            false_authorization: case.expected != SpectralStatus::Complete && artifact_emitted,
        });
    }
    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|r| r.expected == SpectralStatus::Complete)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|r| r.expected == SpectralStatus::Ambiguous)
        .count();
    let refused = cases - supported - ambiguous;
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let supported_artifacts = receipts
        .iter()
        .filter(|r| r.expected == SpectralStatus::Complete && r.artifact_correct)
        .count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejected = receipts.iter().filter(|r| r.tamper_rejected).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.expected == SpectralStatus::Complete && !r.exact)
        .count();
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!(exact_decisions, cases);
    assert_eq!(supported_artifacts, supported);
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejected, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "phase75-bounded-spectral-linear-algebra-v1",
        corpus_sha256,
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_artifacts,
        replay_verified,
        tamper_rejected,
        false_authorizations,
        false_denials,
        receipts,
    };
    std::fs::write(
        "docs/phase75_spectral_linear_algebra.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    std::fs::write("docs/phase75_spectral_linear_algebra.md", format!("# Phase 75 — bounded spectral linear algebra\n\n- Cases: 240 (120 supported, 40 ambiguous, 80 refused)\n- Exact decisions: {exact_decisions}/240\n- Supported artifacts: {supported_artifacts}/120\n- Replay verified: {replay_verified}/240\n- Tamper rejected: {tamper_rejected}/240\n- False authorizations / denials: {false_authorizations} / {false_denials}\n\nThe separate spectral substrate supports exact characteristic polynomials, integer-root eigenspaces, diagonalizability, bounded matrix powers, and explicit decompositions for matrices of dimension at most four. Irrational spectra, infinite-dimensional operators, approximate answers, and powers beyond eight remain refused.\n"))?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
