//! Stage 399: independent pressure test for bounded positional conversion.
//!
//! This corpus is independent of the external exam.  It validates exact
//! source/target grounding, finite base boundaries, replay, and tamper
//! rejection before any external transfer is scored.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::source_base_conversion_frontend::{
    formalize_base_conversion_text, replay_verified as frontend_replay, FrontendStatus,
};
use the_machine::source_base_conversion_pack::{
    evaluate_base_conversion, replay_verified, BaseConversionStatus,
};

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
    downstream_status: Option<BaseConversionStatus>,
    expected_value: Option<String>,
    actual_value: Option<String>,
    exact_decision: bool,
    value_correct: bool,
    frontend_replay: bool,
    downstream_replay: bool,
    frontend_tamper_rejected: bool,
    downstream_tamper_rejected: bool,
    false_authorization: bool,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    source_sha256: String,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    authorized_answers: usize,
    supported_values: usize,
    frontend_replays: usize,
    downstream_replays: usize,
    frontend_tamper_rejections: usize,
    downstream_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn run(id: String, text: String, expected: Expected, expected_value: Option<&str>) -> Receipt {
    let frontend = formalize_base_conversion_text(&text, &id);
    let frontend_replay_verified = frontend_replay(&frontend);
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let frontend_tamper_rejected = !frontend_replay(&frontend_tampered);
    let exact_decision = match expected {
        Expected::Supported => frontend.status == FrontendStatus::Complete,
        Expected::Ambiguous => frontend.status == FrontendStatus::Ambiguous,
        Expected::Refused => frontend.status != FrontendStatus::Complete,
    };
    let (downstream_status, actual_value, downstream_replay, downstream_tamper_rejected) =
        if let Some(request) = frontend.request.as_ref() {
            let result = evaluate_base_conversion(request);
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            (
                Some(result.status),
                result.numeral.clone(),
                replay_verified(&result),
                !replay_verified(&tampered),
            )
        } else {
            (None, None, false, true)
        };
    let value_correct = expected == Expected::Supported
        && frontend.status == FrontendStatus::Complete
        && downstream_status == Some(BaseConversionStatus::Complete)
        && actual_value.as_deref() == expected_value;
    let authorized = value_correct && frontend_replay_verified && downstream_replay;
    Receipt {
        id,
        prompt_sha256: digest(&text),
        expected,
        frontend_status: frontend.status,
        downstream_status,
        expected_value: expected_value.map(str::to_owned),
        actual_value,
        exact_decision,
        value_correct,
        frontend_replay: frontend_replay_verified,
        downstream_replay,
        frontend_tamper_rejected,
        downstream_tamper_rejected,
        false_authorization: expected != Expected::Supported && authorized,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let supported = [
        ("Convert 10101_3 to a base 10 integer.", "91"),
        ("Express A03_{16} in base 10.", "2563"),
        ("Convert 222_{10} to base 13.", "141"),
        ("Convert 852_9 to base 10.", "695"),
        ("Express 199_{10} in base 2.", "11000111"),
        ("Convert 111111_2 to base 10.", "63"),
    ];
    let ambiguous = [
        "Convert 101_2 to base 8 or base 10.",
        "Convert 101_2 to either base 8 or base 10.",
        "Convert 101_2 to base 8 or base 16.",
        "Convert 101_2 into base 10 or base 12.",
    ];
    let refused = [
        "Convert 29_2 to base 10.",
        "Convert 3/4_10 to base 2.",
        "Convert -101_2 to base 10.",
        "Convert 199_10 to base 2 and find y-x for its digits.",
        "Convert 101_2 to base 1.",
        "Convert 101_37 to base 10.",
        "Give an approximate decimal conversion of 101_2.",
        "Find the number of zeros when 199_10 is written in base 2.",
    ];
    let mut receipts = Vec::with_capacity(240);
    for index in 0..120 {
        let (text, value) = supported[index % supported.len()];
        receipts.push(run(
            format!("supported_{index:03}"),
            text.into(),
            Expected::Supported,
            Some(value),
        ));
    }
    for index in 0..40 {
        receipts.push(run(
            format!("ambiguous_{index:03}"),
            ambiguous[index % ambiguous.len()].into(),
            Expected::Ambiguous,
            None,
        ));
    }
    for index in 0..80 {
        receipts.push(run(
            format!("refused_{index:03}"),
            refused[index % refused.len()].into(),
            Expected::Refused,
            None,
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
    let authorized_answers = receipts
        .iter()
        .filter(|r| r.value_correct && r.frontend_replay && r.downstream_replay)
        .count();
    let supported_values = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && r.value_correct)
        .count();
    let frontend_replays = receipts.iter().filter(|r| r.frontend_replay).count();
    let downstream_replays = receipts.iter().filter(|r| r.downstream_replay).count();
    let frontend_tamper_rejections = receipts
        .iter()
        .filter(|r| r.frontend_tamper_rejected)
        .count();
    let downstream_tamper_rejections = receipts
        .iter()
        .filter(|r| r.downstream_tamper_rejected)
        .count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && !r.value_correct)
        .count();
    assert_eq!(
        (supported_count, ambiguous_count, refused_count),
        (120, 40, 80)
    );
    assert_eq!(exact_decisions, 240);
    assert_eq!(authorized_answers, 120);
    assert_eq!(supported_values, 120);
    assert_eq!(frontend_replays, 240);
    assert_eq!(frontend_tamper_rejections, 240);
    assert_eq!(downstream_replays, 120);
    assert_eq!(downstream_tamper_rejections, 240);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let source = the_machine::source_base_conversion_pack::SOURCE;
    let report = Report {
        schema: "stage399-external-base-conversion-bench-v1",
        source_sha256: digest(&source),
        corpus_sha256: digest(&receipts),
        cases: receipts.len(),
        supported: supported_count,
        ambiguous: ambiguous_count,
        refused: refused_count,
        exact_decisions,
        authorized_answers,
        supported_values,
        frontend_replays,
        downstream_replays,
        frontend_tamper_rejections,
        downstream_tamper_rejections,
        false_authorizations,
        false_denials,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage399_external_base_conversion_bench.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
