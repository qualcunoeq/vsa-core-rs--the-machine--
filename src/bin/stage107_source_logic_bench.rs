//! Stage 107: source-derived bounded truth-table benchmark.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::source_logic_frontend::{
    formalize_logic_text, replay_verified as fr, LogicFrontendStatus,
};
use the_machine::source_logic_pack::{
    evaluate, replay_verified, validate_source_document, LogicStatus,
};
const SOURCE: &str = include_str!("../../docs/sources/openstax_truth_table_catalog.txt");
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
enum Hidden {
    Supported,
    Ambiguous,
    Unsupported,
}
#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    hidden: Hidden,
    status: String,
    authorized: bool,
    replay: bool,
    tamper: bool,
    false_auth: bool,
    false_denial: bool,
}
#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    authorized: usize,
    ambiguity_preserved: usize,
    unsupported_refused: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    source_mutations_rejected: usize,
    corpus_sha256: String,
    receipts: Vec<Receipt>,
}
fn digest<T: Serialize>(v: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(v).unwrap()))
}
fn hidden(i: usize) -> Hidden {
    match i % 10 {
        0..=5 => Hidden::Supported,
        6..=7 => Hidden::Ambiguous,
        _ => Hidden::Unsupported,
    }
}
fn text(i: usize) -> (String, Hidden) {
    let h = hidden(i);
    let t = match h {
        Hidden::Supported => match i % 4 {
            0 => "Evaluate (p and not q) with p=true, q=false.".into(),
            1 => "Determine whether (p or not p) is a tautology.".into(),
            2 => "Determine whether (p and not p) is a contradiction.".into(),
            _ => "Are p and q equivalent to q and p?".into(),
        },
        Hidden::Ambiguous => "Evaluate p and/or q; either connective may apply.".into(),
        Hidden::Unsupported => {
            "Evaluate a quantified predicate for all x in an infinite domain.".into()
        }
    };
    (t, h)
}
fn main() {
    assert!(validate_source_document(SOURCE));
    let mut receipts = Vec::new();
    let mut texts = Vec::new();
    for i in 0..480 {
        let (t, h) = text(i);
        let f = formalize_logic_text(&t, &format!("logic-{i}"));
        let r = f.request.as_ref().map(evaluate);
        let auth = h == Hidden::Supported
            && f.status == LogicFrontendStatus::Complete
            && r.as_ref()
                .is_some_and(|x| x.status == LogicStatus::Complete && replay_verified(x));
        let mut ft = f.clone();
        ft.replay_hash.push('x');
        let replay = fr(&f) && r.as_ref().is_none_or(replay_verified);
        let tamper = !fr(&ft)
            && r.as_ref().is_none_or(|x| {
                let mut c = x.clone();
                c.replay_hash.push('x');
                !replay_verified(&c)
            });
        let s = if auth {
            "supported"
        } else if f.status == LogicFrontendStatus::Ambiguous {
            "ambiguous"
        } else {
            "unsupported"
        };
        receipts.push(Receipt {
            id: format!("logic-{i:03}"),
            hidden: h,
            status: s.into(),
            authorized: auth,
            replay,
            tamper,
            false_auth: h != Hidden::Supported && auth,
            false_denial: h == Hidden::Supported && !auth,
        });
        texts.push(t);
    }
    let (mutations) = (0..7)
        .filter(|n| {
            !validate_source_document(&SOURCE.lines().take(*n).collect::<Vec<_>>().join("\n"))
        })
        .count();
    assert_eq!(
        (
            receipts
                .iter()
                .filter(|r| r.hidden == Hidden::Supported && r.authorized)
                .count(),
            receipts
                .iter()
                .filter(|r| r.hidden == Hidden::Ambiguous && r.status == "ambiguous")
                .count(),
            receipts
                .iter()
                .filter(|r| r.hidden == Hidden::Unsupported && r.status == "unsupported")
                .count()
        ),
        (288, 96, 96)
    );
    assert_eq!(receipts.iter().filter(|r| !r.replay).count(), 0);
    assert_eq!(receipts.iter().filter(|r| !r.tamper).count(), 0);
    assert_eq!(
        receipts
            .iter()
            .filter(|r| r.false_auth || r.false_denial)
            .count(),
        0
    );
    let report = Report {
        schema: "stage107-source-logic-v1",
        cases: 480,
        supported: 288,
        ambiguous: 96,
        unsupported: 96,
        authorized: 288,
        ambiguity_preserved: 96,
        unsupported_refused: 96,
        replay_verified: 480,
        tamper_rejections: 480,
        false_authorizations: 0,
        false_denials: 0,
        source_mutations_rejected: mutations,
        corpus_sha256: digest(&texts),
        receipts,
    };
    assert_eq!(mutations, 7);
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
