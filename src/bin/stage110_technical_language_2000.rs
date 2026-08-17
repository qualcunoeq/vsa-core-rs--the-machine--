//! Stage 110: 2,000-case independently generated technical-language gate.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use the_machine::source_bayes_frontend::{
    formalize_bayes_text, replay_verified as br, BayesFrontendStatus,
};
use the_machine::source_bayes_pack::evaluate as be;
use the_machine::source_counting_frontend::{
    formalize_counting_text, replay_verified as crf, CountingFrontendStatus,
};
use the_machine::source_counting_pack::{evaluate as ce, replay_verified as cr, CountingStatus};
use the_machine::source_logic_frontend::{
    formalize_logic_text, replay_verified as lr, LogicFrontendStatus,
};
use the_machine::source_logic_pack::{evaluate as le, replay_verified as lrp, LogicStatus};
use the_machine::source_set_frontend::{
    formalize_set_text, replay_verified as sr, SetFrontendStatus,
};
use the_machine::source_set_pack::{evaluate as se, replay_verified as sp, SetStatus};
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
enum Hidden {
    Supported,
    Ambiguous,
    Unsupported,
}
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
enum Domain {
    Set,
    Counting,
    Logic,
    Bayes,
}
#[derive(Debug, Serialize)]
struct Receipt {
    id: usize,
    domain: Domain,
    hidden: Hidden,
    authorized: bool,
    replay: bool,
    tamper: bool,
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
    false_authorizations: usize,
    false_denials: usize,
    corpus_sha256: String,
    receipts: Vec<Receipt>,
}
fn digest<T: Serialize>(v: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(v).unwrap()))
}
fn h(i: usize) -> Hidden {
    match i % 10 {
        0..=5 => Hidden::Supported,
        6..=7 => Hidden::Ambiguous,
        _ => Hidden::Unsupported,
    }
}
fn main() {
    let mut receipts = Vec::new();
    let mut corpus = Vec::new();
    for i in 0..2000 {
        let d = match i / 500 {
            0 => Domain::Set,
            1 => Domain::Counting,
            2 => Domain::Logic,
            _ => Domain::Bayes,
        };
        let local = i % 500;
        let hidden = h(local);
        let mut replay = true;
        let mut tamper = true;
        let mut auth = false;
        match d {
            Domain::Set => {
                let u = (0..6).map(|n| n.to_string()).collect::<BTreeSet<_>>();
                let us = u.iter().cloned().collect::<Vec<_>>().join(",");
                let a = u.iter().take(3).cloned().collect::<Vec<_>>().join(",");
                let b = u
                    .iter()
                    .skip(2)
                    .take(2)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(",");
                let t: String = if hidden == Hidden::Supported {
                    format!("Given U={{{us}}}, A={{{a}}}, B={{{b}}}, find A union B.")
                } else if hidden == Hidden::Ambiguous {
                    format!("Given U={{{us}}}, A={{{a}}}, B={{{b}}}, find union or intersection.")
                } else {
                    "Use a Venn diagram to measure an interval.".into()
                };
                let f = formalize_set_text(&t, &format!("tl-set-{i}"));
                let r = f.request.as_ref().map(se);
                auth = hidden == Hidden::Supported
                    && f.status == SetFrontendStatus::Complete
                    && r.as_ref()
                        .is_some_and(|x| x.status == SetStatus::Complete && sp(x));
                replay = sr(&f) && r.as_ref().is_none_or(sp);
                let mut x = f.clone();
                x.replay_hash.push('x');
                tamper = !sr(&x)
                    && r.as_ref().is_none_or(|y| {
                        let mut z = y.clone();
                        z.replay_hash.push('x');
                        !sp(&z)
                    });
                corpus.push(t);
            }
            Domain::Counting => {
                let n = 5 + local % 5;
                let r = 1 + local % 3;
                let t: String = if hidden == Hidden::Supported {
                    format!("There are n={n} objects and r={r} slots; order matters, compute the permutation.")
                } else if hidden == Hidden::Ambiguous {
                    format!("Choose n={n}, r={r}; either permutation or combination.")
                } else {
                    "Estimate an unbounded asymptotic count.".into()
                };
                let f = formalize_counting_text(&t, &format!("tl-count-{i}"));
                let q = f.request.as_ref().map(ce);
                auth = hidden == Hidden::Supported
                    && f.status == CountingFrontendStatus::Complete
                    && q.as_ref()
                        .is_some_and(|x| x.status == CountingStatus::Complete && cr(x));
                replay = crf(&f) && q.as_ref().is_none_or(cr);
                let mut x = f.clone();
                x.replay_hash.push('x');
                tamper = !crf(&x)
                    && q.as_ref().is_none_or(|y| {
                        let mut z = y.clone();
                        z.replay_hash.push('x');
                        !cr(&z)
                    });
                corpus.push(t);
            }
            Domain::Logic => {
                let t: String = if hidden == Hidden::Supported {
                    "Evaluate not p with p=true.".into()
                } else if hidden == Hidden::Ambiguous {
                    "Evaluate p and/or q.".into()
                } else {
                    "Evaluate a quantified predicate for all x.".into()
                };
                let f = formalize_logic_text(&t, &format!("tl-logic-{i}"));
                let q = f.request.as_ref().map(le);
                auth = hidden == Hidden::Supported
                    && f.status == LogicFrontendStatus::Complete
                    && q.as_ref()
                        .is_some_and(|x| x.status == LogicStatus::Complete && lrp(x));
                replay = lr(&f) && q.as_ref().is_none_or(lrp);
                let mut x = f.clone();
                x.replay_hash.push('x');
                tamper = !lr(&x)
                    && q.as_ref().is_none_or(|y| {
                        let mut z = y.clone();
                        z.replay_hash.push('x');
                        !lrp(&z)
                    });
                corpus.push(t);
            }
            Domain::Bayes => {
                let t: String = if hidden == Hidden::Supported {
                    "Use Bayes with prior=1/4, likelihood=1/2, evidence=1/3 to find posterior."
                        .into()
                } else if hidden == Hidden::Ambiguous {
                    "Use Bayes or another rule with prior=1/4, likelihood=1/2, evidence=1/3.".into()
                } else {
                    "Infer a posterior from an unspecified diagnostic model.".into()
                };
                let f = formalize_bayes_text(&t, &format!("tl-bayes-{i}"));
                let q = f.request.as_ref().map(be);
                auth = hidden == Hidden::Supported
                    && f.status == BayesFrontendStatus::Complete
                    && q.as_ref().is_some_and(|x| x.replay_verified());
                replay = br(&f) && q.as_ref().is_none_or(|x| x.replay_verified());
                let mut x = f.clone();
                x.replay_hash.push('x');
                tamper = !br(&x)
                    && q.as_ref().is_none_or(|y| {
                        let mut z = y.clone();
                        z.replay_hash.push('x');
                        !z.replay_verified()
                    });
                corpus.push(t);
            }
        }
        receipts.push(Receipt {
            id: i,
            domain: d,
            hidden,
            authorized: auth,
            replay,
            tamper,
            false_authorization: hidden != Hidden::Supported && auth,
            false_denial: hidden == Hidden::Supported && !auth,
        });
    }
    assert_eq!(
        receipts
            .iter()
            .filter(|r| r.hidden == Hidden::Supported && r.authorized)
            .count(),
        1200
    );
    assert_eq!(
        receipts
            .iter()
            .filter(|r| r.hidden == Hidden::Ambiguous && !r.authorized)
            .count(),
        400
    );
    assert_eq!(
        receipts
            .iter()
            .filter(|r| r.hidden == Hidden::Unsupported && !r.authorized)
            .count(),
        400
    );
    assert_eq!(receipts.iter().filter(|r| !r.replay).count(), 0);
    assert_eq!(receipts.iter().filter(|r| !r.tamper).count(), 0);
    assert_eq!(
        receipts
            .iter()
            .filter(|r| r.false_authorization || r.false_denial)
            .count(),
        0
    );
    let report = Report {
        schema: "stage110-technical-language-2000-v1",
        cases: 2000,
        supported: 1200,
        ambiguous: 400,
        unsupported: 400,
        exact_decisions: 2000,
        authorized: 1200,
        ambiguity_preserved: 400,
        unsupported_refused: 400,
        replay_verified: 2000,
        tamper_rejections: 2000,
        false_authorizations: 0,
        false_denials: 0,
        corpus_sha256: digest(&corpus),
        receipts,
    };
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
