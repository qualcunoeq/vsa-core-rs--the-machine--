//! Stage 394: independent pressure test for source-derived progression means.
//!
//! The corpus is authored independently of the frozen external exam.  It
//! tests finite arithmetic-progression endpoint grounding, generic source
//! formula execution, ambiguity preservation, and fail-closed boundaries.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::source_formula_pack::{FormulaResult, FormulaStatus};
use the_machine::source_progression_mean_frontend::{
    formalize_progression_mean_text, replay_verified, FrontendStatus,
};
use the_machine::source_progression_mean_pack::evaluate;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    frontend_status: FrontendStatus,
    downstream_status: Option<FormulaStatus>,
    exact_decision: bool,
    authorized: bool,
    frontend_replay: bool,
    downstream_replay: bool,
    frontend_tamper_rejected: bool,
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
    authorized_answers: usize,
    supported_replays: usize,
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

fn run(id: String, text: String, expected: Expected) -> Receipt {
    let frontend = formalize_progression_mean_text(&text);
    let frontend_replay = replay_verified(&frontend);
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let frontend_tamper_rejected = !replay_verified(&frontend_tampered);
    let exact_decision = match expected {
        Expected::Supported => frontend.frontend.status == FrontendStatus::Complete,
        Expected::Ambiguous => frontend.frontend.status == FrontendStatus::Ambiguous,
        Expected::Refused => frontend.frontend.status != FrontendStatus::Complete,
    };
    let (downstream_status, authorized, downstream_replay, downstream_tamper_rejected) =
        if let Some(request) = frontend.frontend.request.as_ref() {
            let result: FormulaResult = evaluate(request);
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            let authorized = expected == Expected::Supported
                && frontend.frontend.status == FrontendStatus::Complete
                && result.status == FormulaStatus::Complete
                && result.value.is_some()
                && frontend_replay
                && result.replay_verified();
            (
                Some(result.status),
                authorized,
                result.replay_verified(),
                !tampered.replay_verified(),
            )
        } else {
            (None, false, false, true)
        };
    Receipt {
        id,
        expected,
        frontend_status: frontend.frontend.status,
        downstream_status,
        exact_decision,
        authorized,
        frontend_replay,
        downstream_replay,
        frontend_tamper_rejected,
        downstream_tamper_rejected,
        false_authorization: expected != Expected::Supported && authorized,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let supported = [
        "Find the arithmetic progression mean with first=3 and last=27.",
        "The arithmetic sequence has first term -8 and last term 18. What is its mean?",
        "What is the arithmetic mean of all positive two-digit multiples of 7?",
        "Find the average of the multiples of 5 from -20 through 20.",
        "The first term is 11 and the last term is 41. Compute the arithmetic mean.",
        "What is the mean of the multiples of 3 from 4 through 19?",
    ];
    let ambiguous = [
        "Find the arithmetic mean of an arithmetic sequence.",
        "What is the mean of multiples of 7?",
        "The arithmetic progression has first=3. Find its average.",
        "The first term is 3 and the common difference is 2. Find the mean.",
        "Find the average of all multiples of 11.",
    ];
    let refused = [
        "Find the mean of the continuous progression from 1 through 9.",
        "Find the mean of an infinite arithmetic sequence.",
        "Find the weighted mean of the progression with first=1 and last=9.",
        "Find the median of the arithmetic progression with first=1 and last=9.",
        "Find the arithmetic progression mean with first=9 and last=3.",
        "Find the arithmetic progression mean with first=4 and last=4.",
        "Find the mean of the finite list 2, 4, 6, and 8.",
        "The arithmetic progression has first=3 and last=27. Find its variance.",
    ];

    let mut receipts = Vec::with_capacity(240);
    for index in 0..120 {
        receipts.push(run(
            format!("supported_{index:03}"),
            supported[index % supported.len()].into(),
            Expected::Supported,
        ));
    }
    for index in 0..40 {
        receipts.push(run(
            format!("ambiguous_{index:03}"),
            ambiguous[index % ambiguous.len()].into(),
            Expected::Ambiguous,
        ));
    }
    for index in 0..80 {
        receipts.push(run(
            format!("refused_{index:03}"),
            refused[index % refused.len()].into(),
            Expected::Refused,
        ));
    }

    let supported_count = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Supported)
        .count();
    let ambiguous_count = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Ambiguous)
        .count();
    let refused_count = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Refused)
        .count();
    let exact_decisions = receipts
        .iter()
        .filter(|receipt| receipt.exact_decision)
        .count();
    let authorized_answers = receipts.iter().filter(|receipt| receipt.authorized).count();
    let supported_replays = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Supported && receipt.downstream_replay)
        .count();
    let frontend_replays = receipts
        .iter()
        .filter(|receipt| receipt.frontend_replay)
        .count();
    let frontend_tamper_rejections = receipts
        .iter()
        .filter(|receipt| receipt.frontend_tamper_rejected)
        .count();
    let downstream_replays = receipts
        .iter()
        .filter(|receipt| receipt.downstream_replay)
        .count();
    let downstream_tamper_rejections = receipts
        .iter()
        .filter(|receipt| receipt.downstream_tamper_rejected)
        .count();
    let false_authorizations = receipts
        .iter()
        .filter(|receipt| receipt.false_authorization)
        .count();
    let false_denials = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Supported && !receipt.authorized)
        .count();

    assert_eq!(receipts.len(), 240);
    assert_eq!(
        (supported_count, ambiguous_count, refused_count),
        (120, 40, 80)
    );
    assert_eq!(exact_decisions, 240);
    assert_eq!(authorized_answers, 120);
    assert_eq!(supported_replays, 120);
    assert_eq!(frontend_replays, 240);
    assert_eq!(frontend_tamper_rejections, 240);
    assert_eq!(downstream_replays, 120);
    assert_eq!(downstream_tamper_rejections, 240);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);

    let report = Report {
        schema: "stage394-external-progression-mean-bench-v1",
        corpus_sha256: digest(&receipts),
        cases: receipts.len(),
        supported: supported_count,
        ambiguous: ambiguous_count,
        refused: refused_count,
        exact_decisions,
        authorized_answers,
        supported_replays,
        frontend_replays,
        frontend_tamper_rejections,
        downstream_replays,
        downstream_tamper_rejections,
        false_authorizations,
        false_denials,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage394_external_progression_mean_bench.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
