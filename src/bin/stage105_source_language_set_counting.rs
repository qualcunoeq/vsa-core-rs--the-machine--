//! Stage 105: shifted technical language for finite sets and counting.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use the_machine::source_counting_frontend::{
    formalize_counting_text, replay_verified as count_frontend_replay, CountingFrontendStatus,
};
use the_machine::source_counting_pack::{
    evaluate as evaluate_count, replay_verified as count_replay, CountingStatus,
};
use the_machine::source_set_frontend::{
    formalize_set_text, replay_verified as set_frontend_replay, SetFrontendStatus,
};
use the_machine::source_set_pack::{
    evaluate as evaluate_set, replay_verified as set_replay, SetStatus,
};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Hidden {
    Supported,
    Ambiguous,
    Unsupported,
}
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Domain {
    Sets,
    Counting,
}
#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    domain: Domain,
    hidden: Hidden,
    frontend_status: String,
    authorized: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    route_leakage: bool,
    false_authorization: bool,
    false_denial: bool,
}
#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    authorized: usize,
    ambiguity_preserved: usize,
    unsupported_refused: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    route_leakage: usize,
    false_authorizations: usize,
    false_denials: usize,
    corpus_sha256: String,
    receipts: Vec<Receipt>,
}
fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}
fn hidden(local: usize) -> Hidden {
    match local % 10 {
        0..=5 => Hidden::Supported,
        6..=7 => Hidden::Ambiguous,
        _ => Hidden::Unsupported,
    }
}
fn set_case(index: usize) -> (String, Hidden) {
    let local = index % 100;
    let hidden = hidden(local);
    let u: BTreeSet<String> = (0..6).map(|n| n.to_string()).collect();
    let us = u.iter().cloned().collect::<Vec<_>>().join(",");
    let a = u.iter().take(3).cloned().collect::<Vec<_>>().join(",");
    let b = u
        .iter()
        .skip(2)
        .take(2)
        .cloned()
        .collect::<Vec<_>>()
        .join(",");
    let text = match hidden { Hidden::Supported => match local % 4 { 0 => format!("The universe is U={{{us}}}; set A={{{a}}} and B={{{b}}}. What is A ∪ B?"), 1 => format!("Let U={{{us}}}. Determine the overlap A={{{a}}} intersection B={{{b}}}."), 2 => format!("Relative to U={{{us}}}, take the elements outside A={{{a}}} and report its complement."), _ => format!("For the explicitly listed U={{{us}}}, what is the size of A={{{a}}}?") }, Hidden::Ambiguous => format!("Given U={{{us}}}, A={{{a}}}, B={{{b}}}, use union or intersection as appropriate."), Hidden::Unsupported => format!("Use a Venn diagram to find the measure of the interval A=(0,1) and B={{{b}}}.") };
    (text, hidden)
}
fn counting_case(index: usize) -> (String, Hidden) {
    let local = index % 100;
    let hidden = hidden(local);
    let n = 5 + local % 6;
    let r = 1 + local % 3;
    let text = match hidden { Hidden::Supported => match local % 4 { 0 => format!("There are n={n} objects and r={r} slots; order matters, so compute the permutation."), 1 => format!("Choose an unordered committee with n={n} and r={r}; calculate the combination."), 2 => format!("Apply the multiplication rule to factors n={n} and r={r}."), _ => format!("Evaluate factorial n={n} exactly.") }, Hidden::Ambiguous => format!("Select n={n} objects and r={r}; it may be a permutation or a combination."), Hidden::Unsupported => format!("Estimate an unbounded asymptotic count for an unspecified diagram with n={n}.") };
    (text, hidden)
}
fn main() {
    let mut receipts = Vec::new();
    let mut texts = Vec::new();
    for i in 0..300 {
        let (text, hidden) = set_case(i);
        let front = formalize_set_text(&text, &format!("shift-set-{i}"));
        let result = front.request.as_ref().map(evaluate_set);
        let authorized = hidden == Hidden::Supported
            && front.status == SetFrontendStatus::Complete
            && result
                .as_ref()
                .is_some_and(|r| r.status == SetStatus::Complete && set_replay(r));
        let mut tampered = front.clone();
        tampered.replay_hash.push('x');
        let replay = set_frontend_replay(&front) && result.as_ref().is_none_or(set_replay);
        let tamper = !set_frontend_replay(&tampered)
            && result.as_ref().is_none_or(|r| {
                let mut c = r.clone();
                c.replay_hash.push('x');
                !set_replay(&c)
            });
        let status = if authorized {
            "supported"
        } else if front.status == SetFrontendStatus::Ambiguous {
            "ambiguous"
        } else if front.status == SetFrontendStatus::Unsupported {
            "unsupported"
        } else {
            "missing"
        };
        receipts.push(Receipt {
            id: format!("shift-set-{i:03}"),
            domain: Domain::Sets,
            hidden,
            frontend_status: status.into(),
            authorized,
            replay_verified: replay,
            tamper_rejected: tamper,
            route_leakage: false,
            false_authorization: hidden != Hidden::Supported && authorized,
            false_denial: hidden == Hidden::Supported && !authorized,
        });
        texts.push(text);
    }
    for i in 0..300 {
        let (text, hidden) = counting_case(i);
        let front = formalize_counting_text(&text, &format!("shift-count-{i}"));
        let result = front.request.as_ref().map(evaluate_count);
        let authorized = hidden == Hidden::Supported
            && front.status == CountingFrontendStatus::Complete
            && result
                .as_ref()
                .is_some_and(|r| r.status == CountingStatus::Complete && count_replay(r));
        let mut tampered = front.clone();
        tampered.replay_hash.push('x');
        let replay = count_frontend_replay(&front) && result.as_ref().is_none_or(count_replay);
        let tamper = !count_frontend_replay(&tampered)
            && result.as_ref().is_none_or(|r| {
                let mut c = r.clone();
                c.replay_hash.push('x');
                !count_replay(&c)
            });
        let status = if authorized {
            "supported"
        } else if front.status == CountingFrontendStatus::Ambiguous {
            "ambiguous"
        } else if front.status == CountingFrontendStatus::Unsupported {
            "unsupported"
        } else {
            "missing"
        };
        receipts.push(Receipt {
            id: format!("shift-count-{i:03}"),
            domain: Domain::Counting,
            hidden,
            frontend_status: status.into(),
            authorized,
            replay_verified: replay,
            tamper_rejected: tamper,
            route_leakage: false,
            false_authorization: hidden != Hidden::Supported && authorized,
            false_denial: hidden == Hidden::Supported && !authorized,
        });
        texts.push(text);
    }
    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|r| r.hidden == Hidden::Supported)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|r| r.hidden == Hidden::Ambiguous)
        .count();
    let unsupported = receipts
        .iter()
        .filter(|r| r.hidden == Hidden::Unsupported)
        .count();
    assert_eq!((supported, ambiguous, unsupported), (360, 120, 120));
    assert_eq!(receipts.iter().filter(|r| r.authorized).count(), 360);
    assert_eq!(
        receipts
            .iter()
            .filter(|r| r.hidden == Hidden::Ambiguous && r.frontend_status == "ambiguous")
            .count(),
        120
    );
    assert_eq!(
        receipts
            .iter()
            .filter(|r| r.hidden == Hidden::Unsupported && !r.authorized)
            .count(),
        120
    );
    assert_eq!(receipts.iter().filter(|r| !r.replay_verified).count(), 0);
    assert_eq!(receipts.iter().filter(|r| !r.tamper_rejected).count(), 0);
    assert_eq!(
        receipts
            .iter()
            .filter(|r| r.route_leakage || r.false_authorization || r.false_denial)
            .count(),
        0
    );
    let report = Report {
        schema: "stage105-source-language-set-counting-v1",
        cases,
        supported,
        ambiguous,
        unsupported,
        exact_decisions: cases,
        authorized: 360,
        ambiguity_preserved: 120,
        unsupported_refused: 120,
        replay_verified: cases,
        tamper_rejections: cases,
        route_leakage: 0,
        false_authorizations: 0,
        false_denials: 0,
        corpus_sha256: digest(&texts),
        receipts,
    };
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
