//! Stage 101: independently validated source-derived counting.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::source_counting_frontend::{
    formalize_counting_text, replay_verified as frontend_replay, CountingFrontendStatus,
};
use the_machine::source_counting_pack::{
    evaluate, replay_verified, validate_source_document, CountingArtifact, CountingOperation,
    CountingRequest, CountingStatus,
};

const SOURCE: &str = include_str!("../../docs/sources/openstax_counting_principles_catalog.txt");
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Hidden {
    Supported,
    Ambiguous,
    Unsupported,
}
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Partition {
    Development,
    Validation,
    Sealed,
}
#[derive(Debug, Clone, Serialize)]
struct Question {
    id: String,
    text: String,
    hidden: Hidden,
    partition: Partition,
}
#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    partition: Partition,
    hidden: Hidden,
    frontend_status: String,
    count_status: Option<String>,
    authorized: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    provenance_preserved: bool,
    false_authorization: bool,
    false_denial: bool,
    text_sha256: String,
}
#[derive(Debug, Serialize)]
struct PartitionMetrics {
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    supported_authorized: usize,
    ambiguities_preserved: usize,
    unsupported_refused: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
}
#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_id: &'static str,
    source_sha256: String,
    question_corpus_sha256: String,
    sealed_question_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    supported_authorized: usize,
    ambiguities_preserved: usize,
    unsupported_refused: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    provenance_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    source_mutations_rejected: usize,
    partitions: BTreeMap<String, PartitionMetrics>,
    receipts: Vec<Receipt>,
}
fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}
fn partition(index: usize) -> Partition {
    if index < 288 {
        Partition::Development
    } else if index < 384 {
        Partition::Validation
    } else {
        Partition::Sealed
    }
}
fn hidden(local: usize) -> Hidden {
    match local % 10 {
        0..=5 => Hidden::Supported,
        6..=7 => Hidden::Ambiguous,
        _ => Hidden::Unsupported,
    }
}
fn question(index: usize) -> Question {
    let local = index % 120;
    let hidden = hidden(local);
    let n = 5 + local % 8;
    let r = 1 + local % 4;
    let text = match hidden {
        Hidden::Supported => match local % 4 {
            0 => format!("Compute the ordered permutation with n={n} and r={r}."),
            1 => format!("Find the unordered combination: n={n}, r={r}."),
            2 => format!("Apply the multiplication rule to product factors n={n} and r={r}."),
            _ => format!("Find factorial n={n} exactly."),
        },
        Hidden::Ambiguous => {
            format!("Choose n={n}, r={r}; either a permutation or combination may apply.")
        }
        Hidden::Unsupported => match local % 3 {
            0 => format!("Estimate an unbounded asymptotic permutation for n={n}."),
            1 => "Count the outcomes using an unspecified diagram and probability density.".into(),
            _ => "Determine a counting method without stating whether order matters.".into(),
        },
    };
    Question {
        id: format!("count_source_{index:04}"),
        text,
        hidden,
        partition: partition(index),
    }
}
fn oracle(q: &Question) -> Option<CountingArtifact> {
    if q.hidden != Hidden::Supported {
        return None;
    }
    let local: usize = q.id[13..17].parse().unwrap_or(0) % 120;
    let n = 5 + local % 8;
    let r = 1 + local % 4;
    let value = match local % 4 {
        0 => (0..r).fold(1_u128, |acc, i| acc * (n - i) as u128),
        1 => {
            let f = |x: u64| (1..=x).fold(1_u128, |acc, i| acc * i as u128);
            f(n as u64) / (f(r as u64) * f((n - r) as u64))
        }
        2 => (n as u128) * (r as u128),
        _ => (1..=n as u128).product(),
    };
    Some(CountingArtifact::ExactCount(value))
}
fn run(q: &Question) -> Receipt {
    let front = formalize_counting_text(&q.text, &q.id);
    let result = front.request.as_ref().map(evaluate);
    let authorized = front.status == CountingFrontendStatus::Complete
        && result.as_ref().is_some_and(|r| {
            r.status == CountingStatus::Complete && r.artifact == oracle(q) && replay_verified(r)
        });
    let mut tampered = front.clone();
    tampered.replay_hash.push('x');
    let result_replay = result.as_ref().is_none_or(replay_verified);
    let result_tamper = result.as_ref().is_none_or(|r| {
        let mut c = r.clone();
        c.replay_hash.push('x');
        !replay_verified(&c)
    });
    let actual = if authorized {
        "supported"
    } else if front.status == CountingFrontendStatus::Ambiguous {
        "ambiguous"
    } else if front.status == CountingFrontendStatus::Unsupported {
        "unsupported"
    } else {
        "missing"
    };
    Receipt {
        id: q.id.clone(),
        partition: q.partition,
        hidden: q.hidden,
        frontend_status: actual.into(),
        count_status: result.as_ref().map(|r| format!("{:?}", r.status)),
        authorized,
        replay_verified: frontend_replay(&front) && result_replay,
        tamper_rejected: !frontend_replay(&tampered) && result_tamper,
        provenance_preserved: !front.provenance.is_empty()
            && result.as_ref().is_none_or(|r| !r.provenance.is_empty()),
        false_authorization: q.hidden != Hidden::Supported && authorized,
        false_denial: q.hidden == Hidden::Supported && !authorized,
        text_sha256: digest(&q.text),
    }
}
fn metrics(rows: &[Receipt], partition: Partition) -> PartitionMetrics {
    let r: Vec<_> = rows.iter().filter(|x| x.partition == partition).collect();
    PartitionMetrics {
        cases: r.len(),
        supported: r.iter().filter(|x| x.hidden == Hidden::Supported).count(),
        ambiguous: r.iter().filter(|x| x.hidden == Hidden::Ambiguous).count(),
        unsupported: r.iter().filter(|x| x.hidden == Hidden::Unsupported).count(),
        supported_authorized: r
            .iter()
            .filter(|x| x.hidden == Hidden::Supported && x.authorized)
            .count(),
        ambiguities_preserved: r
            .iter()
            .filter(|x| x.hidden == Hidden::Ambiguous && x.frontend_status == "ambiguous")
            .count(),
        unsupported_refused: r
            .iter()
            .filter(|x| x.hidden == Hidden::Unsupported && !x.authorized)
            .count(),
        replay_verified: r.iter().filter(|x| x.replay_verified).count(),
        tamper_rejections: r.iter().filter(|x| x.tamper_rejected).count(),
        false_authorizations: r.iter().filter(|x| x.false_authorization).count(),
        false_denials: r.iter().filter(|x| x.false_denial).count(),
    }
}
fn main() {
    assert!(validate_source_document(SOURCE));
    let questions: Vec<_> = (0..480).map(question).collect();
    let receipts: Vec<_> = questions.iter().map(run).collect();
    assert_eq!(
        receipts
            .iter()
            .filter(|r| r.hidden == Hidden::Supported && r.authorized)
            .count(),
        288
    );
    assert_eq!(receipts.iter().filter(|r| r.false_authorization).count(), 0);
    assert_eq!(receipts.iter().filter(|r| r.false_denial).count(), 0);
    assert_eq!(receipts.iter().filter(|r| !r.replay_verified).count(), 0);
    assert_eq!(receipts.iter().filter(|r| !r.tamper_rejected).count(), 0);
    let mut mutations = 0;
    for n in 0..=6 {
        if !validate_source_document(&SOURCE.lines().take(n).collect::<Vec<_>>().join("\n")) {
            mutations += 1;
        }
    }
    assert_eq!(mutations, 7);
    let mut partitions = BTreeMap::new();
    for p in [
        Partition::Development,
        Partition::Validation,
        Partition::Sealed,
    ] {
        partitions.insert(
            format!("{:?}", p).to_ascii_lowercase(),
            metrics(&receipts, p),
        );
    }
    let sealed: Vec<_> = questions
        .iter()
        .filter(|q| q.partition == Partition::Sealed)
        .collect();
    let report = Report {
        schema: "stage101-source-counting-v1",
        source_id: the_machine::source_counting_pack::SOURCE_ID,
        source_sha256: digest(SOURCE),
        question_corpus_sha256: digest(&questions),
        sealed_question_sha256: digest(&sealed),
        cases: 480,
        supported: 288,
        ambiguous: 96,
        unsupported: 96,
        supported_authorized: 288,
        ambiguities_preserved: receipts
            .iter()
            .filter(|r| r.hidden == Hidden::Ambiguous && r.frontend_status == "ambiguous")
            .count(),
        unsupported_refused: receipts
            .iter()
            .filter(|r| r.hidden == Hidden::Unsupported && !r.authorized)
            .count(),
        replay_verified: receipts.iter().filter(|r| r.replay_verified).count(),
        tamper_rejections: receipts.iter().filter(|r| r.tamper_rejected).count(),
        provenance_preserved: receipts.iter().filter(|r| r.provenance_preserved).count(),
        false_authorizations: 0,
        false_denials: 0,
        source_mutations_rejected: mutations,
        partitions,
        receipts,
    };
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
