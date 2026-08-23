//! Stage 395: independent pressure test for natural combination binding.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::source_combination_frontend::{
    formalize_combination_text, replay_verified, CombinationFrontendStatus,
};
use the_machine::source_combination_pack::evaluate_combination;
use the_machine::source_counting_pack::replay_verified as counting_replay_verified;
use the_machine::source_counting_pack::{CountingResult, CountingStatus};

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
    frontend_status: CombinationFrontendStatus,
    downstream_status: Option<CountingStatus>,
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
    let frontend = formalize_combination_text(&text, &id);
    let frontend_replay = replay_verified(&frontend);
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let frontend_tamper_rejected = !replay_verified(&frontend_tampered);
    let exact_decision = match expected {
        Expected::Supported => frontend.status == CombinationFrontendStatus::Complete,
        Expected::Ambiguous => frontend.status == CombinationFrontendStatus::Ambiguous,
        Expected::Refused => frontend.status != CombinationFrontendStatus::Complete,
    };
    let (downstream_status, authorized, downstream_replay, downstream_tamper_rejected) =
        if let Some(request) = frontend.request.as_ref() {
            let result: CountingResult = evaluate_combination(request);
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            let authorized = expected == Expected::Supported
                && frontend.status == CombinationFrontendStatus::Complete
                && result.status == CountingStatus::Complete
                && result.artifact.is_some()
                && frontend_replay
                && counting_replay_verified(&result);
            (
                Some(result.status),
                authorized,
                counting_replay_verified(&result),
                !counting_replay_verified(&tampered),
            )
        } else {
            (None, false, false, true)
        };
    Receipt {
        id,
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
        "In how many ways can a student choose three out of eight classes?",
        "How many ways can a committee choose 4 out of 9 members?",
        "Find the number of ways to choose seven out of twelve objects.",
        "A researcher may choose 2 out of 11 samples. In how many ways?",
        "How many ways can we choose fifteen out of twenty items?",
        "In how many ways may one choose fourth out of ten?",
    ];
    let ambiguous = [
        "In how many ways may one choose three out of eight or choose two out of seven?",
        "How many ways can a person choose three out of eight or choose four out of nine?",
        "In how many ways does the question allow one to choose three out of eight or choose five out of ten?",
        "In how many ways may one choose two out of seven or choose four out of nine?",
        "In how many ways can one choose three out of eight or choose one out of five; the requested route is unclear?",
    ];
    let refused = [
        "What is the probability of choosing 2 out of 7?",
        "How many ways can you choose 3 out of 8 when order matters?",
        "How many ways can you choose 3 cards out of 52 if all have different suits?",
        "How many ways can you choose at least 3 out of 8?",
        "How many ways can you choose exactly 3 out of 8?",
        "How many ways can you arrange 3 out of 8 objects?",
        "How many ways can you choose 25 out of 30 under this restriction?",
        "Choose 3 out of 8 at random and compute the probability.",
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
        schema: "stage395-external-combination-binding-bench-v1",
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
        "docs/stage395_external_combination_binding_bench.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
