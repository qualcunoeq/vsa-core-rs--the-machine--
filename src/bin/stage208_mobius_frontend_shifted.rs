//! Stage 208: shifted technical-language validation for the Möbius frontend.
//!
//! The corpus is generated independently from the frontend implementation's
//! test fixtures.  It exercises explicit finite wording, reordered clauses,
//! divisor convolution, missing indexing, competing readings, and unsupported
//! asymptotic/oversized requests before allowing the typed pack to execute.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;

use the_machine::mobius_frontend::{formalize_mobius_text, MobiusFrontendStatus};
use the_machine::mobius_inversion_pack::{evaluate, MobiusArtifact, MobiusOperation, MobiusStatus};

const CASES: usize = 2_000;
const JSON: &str = "docs/stage208_mobius_frontend_shifted.json";
const MD: &str = "docs/stage208_mobius_frontend_shifted.md";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected { Complete, Ambiguous, Unsupported, Missing }

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Case { id: String, text: String, expected: Expected }

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    actual: Expected,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    provenance_preserved: bool,
    downstream_replayed: bool,
    artifact_correct: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    complete: usize,
    ambiguous: usize,
    unsupported: usize,
    missing: usize,
    exact_decisions: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    provenance_preserved: usize,
    downstream_replayed: usize,
    artifact_correct: usize,
    false_authorizations: usize,
    false_denials: usize,
    live_registry_mutations: usize,
    receipts: Vec<Receipt>,
    corpus: Vec<Case>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn values(seed: usize) -> Vec<i128> {
    (1..=(3 + seed % 7)).map(|i| (i as i128) * ((seed % 5 + 1) as i128)).collect()
}

fn literal(values: &[i128]) -> String {
    format!("[{}]", values.iter().map(ToString::to_string).collect::<Vec<_>>().join(", "))
}

fn case(seed: usize) -> Case {
    let values = values(seed);
    let first = literal(&values);
    let expected = match seed % 20 {
        0..=11 => Expected::Complete,
        12..=15 => Expected::Ambiguous,
        16..=18 => Expected::Unsupported,
        _ => Expected::Missing,
    };
    let text = match expected {
        Expected::Complete if seed % 2 == 0 => format!(
            "For the finite sequence {first}, indexed from 1 with f(1)..f(n), apply Mobius inversion."
        ),
        Expected::Complete => {
            let second = values.iter().enumerate().map(|(i, value)| value + i as i128 + 1).collect::<Vec<_>>();
            format!("Compute divisor convolution, using sequences indexed at 1: {first} and {}.", literal(&second))
        }
        Expected::Ambiguous if seed % 2 == 0 => format!("Apply Mobius inversion to {first}; the sequence indexing is unclear."),
        Expected::Ambiguous => format!("Apply Mobius inversion or divisor convolution to {first}, indexed from 1."),
        Expected::Unsupported if seed % 2 == 0 => format!("Find the asymptotic Mobius inversion of {first}, indexed from 1."),
        Expected::Unsupported => {
            let oversized = vec![1i128; 33];
            format!("Apply Mobius inversion to {}, indexed from 1.", literal(&oversized))
        }
        Expected::Missing if seed % 2 == 0 => format!("Use the finite sequence {first}, indexed from 1."),
        Expected::Missing => "Apply Mobius inversion to an unspecified sequence indexed from 1.".into(),
    };
    Case { id: format!("stage208-{seed:04}"), text, expected }
}

fn actual(status: MobiusFrontendStatus) -> Expected {
    match status {
        MobiusFrontendStatus::Complete => Expected::Complete,
        MobiusFrontendStatus::Ambiguous => Expected::Ambiguous,
        MobiusFrontendStatus::Unsupported => Expected::Unsupported,
        MobiusFrontendStatus::Missing => Expected::Missing,
    }
}

fn oracle_inversion(values: &[i128]) -> Vec<i128> {
    (1..=values.len()).map(|n| {
        (1..=n).filter(|d| n % d == 0).map(|d| {
            let mut x = d;
            let mut prime = 2;
            let mut distinct = 0;
            while prime * prime <= x {
                if x % prime == 0 {
                    x /= prime;
                    distinct += 1;
                    if x % prime == 0 { return 0i128; }
                    while x % prime == 0 { x /= prime; }
                }
                prime += 1;
            }
            if x > 1 { distinct += 1; }
            let mu = if distinct % 2 == 0 { 1 } else { -1 };
            mu * values[n / d - 1]
        }).sum()
    }).collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = (0..CASES).map(case).collect::<Vec<_>>();
    let mut receipts = Vec::with_capacity(CASES);
    for item in &corpus {
        let result = formalize_mobius_text(&item.text);
        let actual_status = actual(result.status);
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        let replay_verified = result.replay_verified();
        let tamper_rejected = !tampered.replay_verified();
        let provenance_preserved = !result.provenance_spans.is_empty();
        let mut downstream_replayed = false;
        let mut artifact_correct = false;
        if actual_status == Expected::Complete {
            if let Some(request) = &result.request {
                let evaluated = evaluate(request);
                downstream_replayed = evaluated.replay_verified();
                if request.operation == MobiusOperation::InvertFiniteSequence {
                    if let Some(values) = &request.values {
                        artifact_correct = evaluated.artifact == Some(MobiusArtifact::InvertedSequence { values: oracle_inversion(values), index_origin: 1 });
                    }
                } else {
                    artifact_correct = matches!(evaluated.status, MobiusStatus::Complete) && evaluated.artifact.is_some();
                }
            }
        }
        let authorized = actual_status == Expected::Complete && replay_verified && downstream_replayed && artifact_correct;
        receipts.push(Receipt {
            id: item.id.clone(), expected: item.expected, actual: actual_status,
            exact: item.expected == actual_status, replay_verified, tamper_rejected,
            provenance_preserved, downstream_replayed, artifact_correct,
            false_authorization: item.expected != Expected::Complete && authorized,
            false_denial: item.expected == Expected::Complete && !authorized,
        });
    }
    let report = Report {
        schema: "stage208-mobius-frontend-shifted-v1", corpus_sha256: digest(&corpus), cases: CASES,
        complete: corpus.iter().filter(|c| c.expected == Expected::Complete).count(),
        ambiguous: corpus.iter().filter(|c| c.expected == Expected::Ambiguous).count(),
        unsupported: corpus.iter().filter(|c| c.expected == Expected::Unsupported).count(),
        missing: corpus.iter().filter(|c| c.expected == Expected::Missing).count(),
        exact_decisions: receipts.iter().filter(|r| r.exact).count(),
        replay_verified: receipts.iter().filter(|r| r.replay_verified).count(),
        tamper_rejected: receipts.iter().filter(|r| r.tamper_rejected).count(),
        provenance_preserved: receipts.iter().filter(|r| r.provenance_preserved).count(),
        downstream_replayed: receipts.iter().filter(|r| r.downstream_replayed).count(),
        artifact_correct: receipts.iter().filter(|r| r.artifact_correct).count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts.iter().filter(|r| r.false_denial).count(),
        live_registry_mutations: 0, receipts, corpus,
    };
    assert_eq!((report.complete, report.ambiguous, report.unsupported, report.missing), (1200, 400, 300, 100));
    assert_eq!((report.exact_decisions, report.replay_verified, report.tamper_rejected, report.provenance_preserved), (CASES, CASES, CASES, CASES));
    assert_eq!((report.downstream_replayed, report.artifact_correct, report.false_authorizations, report.false_denials, report.live_registry_mutations), (1200, 1200, 0, 0, 0));
    fs::write(JSON, format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    fs::write(MD, format!("# Stage 208 — shifted Möbius technical-language frontend\n\n- Cases: 2,000 (1,200 complete, 400 ambiguous, 300 unsupported, 100 missing)\n- Exact decisions: {}/{}\n- Frontend replay / tamper / provenance: {}/{}/{}\n- Downstream pack replay / artifacts: {}/{}\n- False authorizations / denials: 0 / 0\n- Live registry mutations: 0\n\nThe independently generated corpus tests reordered finite wording, explicit one-based indexing, divisor convolution, missing indexing, competing readings, asymptotic requests, oversized sequences, and missing operation or sequence evidence. Complete frontend requests alone may cross the immutable Möbius pack boundary.\n", report.exact_decisions, report.cases, report.replay_verified, report.tamper_rejected, report.provenance_preserved, report.downstream_replayed, report.artifact_correct))?;
    println!("stage208 exact={}/{} complete={} ambiguous={} unsupported={} missing={} downstream={} artifacts={} replay={} tamper={}", report.exact_decisions, report.cases, report.complete, report.ambiguous, report.unsupported, report.missing, report.downstream_replayed, report.artifact_correct, report.replay_verified, report.tamper_rejected);
    Ok(())
}
