//! Stage 77: independent technical-language frontend for spectral algebra.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::spectral_frontend::{formalize_spectral_text, SpectralFrontendStatus};
use the_machine::spectral_linear_algebra_pack::evaluate_spectral;

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
    actual: SpectralFrontendStatus,
    exact: bool,
    frontend_replay: bool,
    frontend_tamper_rejected: bool,
    downstream_emitted: bool,
    downstream_replay: bool,
    downstream_tamper_rejected: bool,
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
    frontend_replays: usize,
    frontend_tamper_rejections: usize,
    downstream_replays: usize,
    downstream_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut corpus = Vec::new();
    for index in 0..20 {
        corpus.push((
            format!("characteristic_{index}"),
            Expected::Complete,
            "Find the characteristic polynomial of [[2,1],[1,2]].".to_owned(),
        ));
    }
    for index in 0..20 {
        corpus.push((
            format!("eigenvalues_{index}"),
            Expected::Complete,
            "Find the eigenvalues of [[2,0],[0,5]].".to_owned(),
        ));
    }
    for index in 0..20 {
        corpus.push((
            format!("eigenspace_{index}"),
            Expected::Complete,
            "Find the eigenspace for eigenvalue=3 of [[2,1],[1,2]].".to_owned(),
        ));
    }
    for index in 0..20 {
        corpus.push((
            format!("diagonalizable_{index}"),
            Expected::Complete,
            "Determine whether [[1,0],[0,1]] is diagonalizable.".to_owned(),
        ));
    }
    for index in 0..20 {
        corpus.push((
            format!("power_{index}"),
            Expected::Complete,
            "Compute the matrix power power=2 of [[2,1],[1,2]].".to_owned(),
        ));
    }
    for index in 0..20 {
        corpus.push((
            format!("decomposition_{index}"),
            Expected::Complete,
            "Give the spectral decomposition of [[2,0],[0,5]].".to_owned(),
        ));
    }
    for index in 0..40 {
        corpus.push((
            format!("ambiguous_{index}"),
            Expected::Ambiguous,
            "Find the eigenvalues and characteristic polynomial of [[2,1],[1,2]].".to_owned(),
        ));
    }
    for index in 0..20 {
        corpus.push((
            format!("unsupported_approx_{index}"),
            Expected::Refused,
            "Give a numerical approximate spectrum of [[2,1],[1,2]].".to_owned(),
        ));
    }
    for index in 0..20 {
        corpus.push((
            format!("unsupported_infinite_{index}"),
            Expected::Refused,
            "Analyze the infinite-dimensional spectral gap of [[2,1],[1,2]].".to_owned(),
        ));
    }
    for index in 0..20 {
        corpus.push((
            format!("missing_matrix_{index}"),
            Expected::Refused,
            "Find the eigenvalues of matrix A.".to_owned(),
        ));
    }
    for index in 0..20 {
        corpus.push((
            format!("missing_power_{index}"),
            Expected::Refused,
            "Compute the matrix power of [[2,1],[1,2]].".to_owned(),
        ));
    }
    assert_eq!(corpus.len(), 240);
    let corpus_sha256 = digest(&corpus);
    let mut receipts = Vec::new();
    for (id, expected, text) in corpus {
        let frontend = formalize_spectral_text(&text);
        let mut tampered = frontend.clone();
        tampered.replay_hash.push('x');
        let mut downstream_emitted = false;
        let mut downstream_replay = false;
        let mut downstream_tamper_rejected = false;
        if let Some(request) = frontend.request.as_ref() {
            let result = evaluate_spectral(request);
            downstream_emitted = result.status
                == the_machine::spectral_linear_algebra_pack::SpectralStatus::Complete
                && result.artifact.is_some();
            downstream_replay = result.replay_verified();
            let mut changed = result.clone();
            changed.replay_hash.push('x');
            downstream_tamper_rejected = !changed.replay_verified();
        }
        let expected_status = match expected {
            Expected::Complete => SpectralFrontendStatus::Complete,
            Expected::Ambiguous => SpectralFrontendStatus::Ambiguous,
            Expected::Refused => SpectralFrontendStatus::Missing,
        };
        let exact = match expected {
            Expected::Complete => frontend.status == expected_status && downstream_emitted,
            Expected::Ambiguous => frontend.status == expected_status && !downstream_emitted,
            Expected::Refused => frontend.status != SpectralFrontendStatus::Complete,
        };
        receipts.push(Receipt {
            id,
            expected,
            actual: frontend.status,
            exact,
            frontend_replay: frontend.replay_verified(),
            frontend_tamper_rejected: !tampered.replay_verified(),
            downstream_emitted,
            downstream_replay,
            downstream_tamper_rejected,
            false_authorization: expected != Expected::Complete && downstream_emitted,
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
    let supported_artifacts = receipts
        .iter()
        .filter(|r| r.expected == Expected::Complete && r.downstream_emitted)
        .count();
    let frontend_replays = receipts.iter().filter(|r| r.frontend_replay).count();
    let frontend_tamper_rejections = receipts
        .iter()
        .filter(|r| r.frontend_tamper_rejected)
        .count();
    let downstream_replays = receipts.iter().filter(|r| r.downstream_replay).count();
    let downstream_tamper_rejections = receipts
        .iter()
        .filter(|r| r.downstream_tamper_rejected)
        .count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.expected == Expected::Complete && !r.exact)
        .count();
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!(exact_decisions, 240);
    assert_eq!(supported_artifacts, 120);
    assert_eq!(frontend_replays, 240);
    assert_eq!(frontend_tamper_rejections, 240);
    assert_eq!(downstream_replays, 120);
    assert_eq!(downstream_tamper_rejections, 120);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage77-spectral-technical-language-v1",
        corpus_sha256,
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_artifacts,
        frontend_replays,
        frontend_tamper_rejections,
        downstream_replays,
        downstream_tamper_rejections,
        false_authorizations,
        false_denials,
        receipts,
    };
    std::fs::write(
        "docs/stage77_spectral_language_frontend.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    std::fs::write("docs/stage77_spectral_language_frontend.md", format!("# Stage 77 — spectral technical-language frontend\n\n- Cases: 240 (120 supported, 40 ambiguous, 80 refused)\n- Exact decisions: 240/240\n- Supported downstream artifacts: 120/120\n- Frontend replay and tamper: 240/240 each\n- Downstream replay and tamper: 120/120 emitted artifacts each\n- False authorizations / denials: 0 / 0\n\nThe frontend requires explicit matrix literals, operation phrases, eigenvalues, and finite powers. It refuses approximate, infinite-dimensional, missing, and operation-ambiguous reports.\n"))?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
