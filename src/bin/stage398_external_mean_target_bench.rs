//! Stage 398: independent pressure test for stated-mean/one-unknown binding.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::source_formula_pack::FormulaStatus;
use the_machine::source_statistics_frontend::{formalize_finite_list_mean_text, FrontendStatus};
use the_machine::source_statistics_pack::evaluate_statistics;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Serialize)]
struct Receipt {
    id: String,
    prompt_sha256: String,
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
    let frontend = formalize_finite_list_mean_text(&text);
    let frontend_replay = frontend.replay_verified();
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let frontend_tamper_rejected = !frontend_tampered.replay_verified();
    let exact_decision = match expected {
        Expected::Supported => frontend.status == FrontendStatus::Complete,
        Expected::Ambiguous => frontend.status == FrontendStatus::Ambiguous,
        Expected::Refused => frontend.status != FrontendStatus::Complete,
    };
    let (downstream_status, authorized, downstream_replay, downstream_tamper_rejected) =
        if let Some(request) = frontend.request.as_ref() {
            let result = evaluate_statistics(request);
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            let authorized = expected == Expected::Supported
                && frontend.status == FrontendStatus::Complete
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
        prompt_sha256: digest(&text),
        expected,
        frontend_status: frontend.status,
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
        "Given that 10 is the arithmetic mean of the set {6, 13, 18, 4, x}, what is x?",
        "The average age of the three Wilson children is 7 years. If the two younger children are 4 years old and 7 years old, how many years old is the oldest child?",
        "The mean of the set {4, 9, 11, y} is 8. Find y.",
        "If 12 is the mean of five values {3, 8, 10, 14, z}, determine z.",
        "The average of the four readings 2, 5, 9, and q is 7. What is q?",
        "The average age of four students is 15 years. Three ages are 12, 14, and 17 years. Find the remaining age.",
    ];
    let ambiguous = [
        "The average of three values is 7, but only one value is provided.",
        "The arithmetic mean of a finite set is requested, but its contents are not listed.",
        "The average age of three children is seven years, with no ages supplied.",
        "The mean of four values is stated, but the number of known values is unclear.",
        "A finite list has one missing value and a stated average, but the list itself is omitted.",
    ];
    let refused = [
        "The median of {6, 13, 18, 4, x} is 10. Find x.",
        "The minimum average of the set {6, 13, 18, 4, x} is 10. Find x.",
        "Find the weighted average when the mean of {4, 9, x} is 8.",
        "The average is shown in a graph and the missing value must be inferred.",
        "The average age of three children is 7, and two ages are known, but the question asks for the median.",
        "The mean of {4, 9, x, y} is 8; determine both unknowns using a general symbolic solver.",
        "A probability distribution has an average of 7 and one unknown probability-weighted value.",
        "The average speed over several time intervals is 7; determine a missing interval speed.",
        "The average of 23 and x is 27. Find the positive difference between 23 and x.",
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
        .filter(|r| r.expected == Expected::Supported)
        .count();
    let ambiguous_count = receipts
        .iter()
        .filter(|r| r.expected == Expected::Ambiguous)
        .count();
    let refused_count = receipts
        .iter()
        .filter(|r| r.expected == Expected::Refused)
        .count();
    let exact_decisions = receipts.iter().filter(|r| r.exact_decision).count();
    for receipt in receipts.iter().filter(|r| !r.exact_decision) {
        eprintln!(
            "mismatch {} expected={:?} actual={:?}",
            receipt.id, receipt.expected, receipt.frontend_status
        );
    }
    let authorized_answers = receipts.iter().filter(|r| r.authorized).count();
    let supported_replays = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && r.downstream_replay)
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
        .filter(|r| r.expected == Expected::Supported && !r.authorized)
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
        schema: "stage398-external-mean-target-bench-v1",
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
        "docs/stage398_external_mean_target_bench.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
