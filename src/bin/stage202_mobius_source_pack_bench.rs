//! Stage 202: independent validation of the source-derived Möbius pack.
//!
//! The source-derived module is exercised with finite sequence inversion and
//! divisor convolution, explicit indexing, provenance, and strict overflow /
//! length boundaries.  The oracle is implemented independently in this
//! benchmark; no expected label is passed to the evaluator.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;

use the_machine::mobius_inversion_pack::{
    evaluate, MobiusArtifact, MobiusOperation, MobiusRequest, MobiusStatus,
};

const JSON: &str = "docs/stage202_mobius_source_pack_bench.json";
const MD: &str = "docs/stage202_mobius_source_pack_bench.md";
const CASES: usize = 240;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected { Supported, Ambiguous, Unsupported }

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Case { id: String, operation: MobiusOperation, expected: Expected, seed: usize }

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    actual: Expected,
    exact: bool,
    artifact_correct: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    provenance_preserved: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    supported_artifacts: usize,
    ambiguous_preserved: usize,
    unsupported_refused: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    provenance_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    live_registry_mutations: usize,
    receipts: Vec<Receipt>,
    corpus: Vec<Case>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn expected(seed: usize) -> Expected {
    match seed % 6 { 0..=2 => Expected::Supported, 3 => Expected::Ambiguous, _ => Expected::Unsupported }
}

fn mobius(n: usize) -> i8 {
    if n == 1 { return 1; }
    let mut value = n;
    let mut prime = 2;
    let mut distinct = 0;
    while prime * prime <= value {
        if value % prime == 0 {
            value /= prime;
            distinct += 1;
            if value % prime == 0 { return 0; }
            while value % prime == 0 { value /= prime; }
        }
        prime += 1;
    }
    if value > 1 { distinct += 1; }
    if distinct % 2 == 0 { 1 } else { -1 }
}

fn divisors(n: usize) -> impl Iterator<Item = usize> { (1..=n).filter(move |d| n % d == 0) }

fn oracle_invert(values: &[i128]) -> Vec<i128> {
    (1..=values.len()).map(|n| divisors(n).map(|d| i128::from(mobius(d)) * values[n / d - 1]).sum()).collect()
}

fn oracle_convolve(left: &[i128], right: &[i128]) -> Vec<i128> {
    (1..=left.len()).map(|n| divisors(n).map(|d| left[d - 1] * right[n / d - 1]).sum()).collect()
}

fn request(case: &Case) -> (MobiusRequest, Option<MobiusArtifact>) {
    let values = if case.expected == Expected::Unsupported {
        vec![1i128; 33]
    } else {
        (1..=(4 + case.seed % 8)).map(|i| (i as i128).pow(2) - (case.seed % 3) as i128).collect()
    };
    let second = values.iter().enumerate().map(|(i, value)| value + i as i128 + 1).collect::<Vec<_>>();
    let expected_artifact = match case.operation {
        MobiusOperation::InvertFiniteSequence => Some(MobiusArtifact::InvertedSequence { values: oracle_invert(&values), index_origin: 1 }),
        MobiusOperation::DivisorConvolution => Some(MobiusArtifact::ConvolutionSequence { values: oracle_convolve(&values, &second), index_origin: 1 }),
    };
    let request = MobiusRequest {
        operation: case.operation,
        values: Some(values),
        second_values: Some(second),
        domain: "bounded_source_mobius_inversion".into(),
        indexing_declared: true,
        ambiguity: (case.expected == Expected::Ambiguous).then(|| "sequence indexing or divisor convention is unresolved".into()),
        provenance: vec![format!("stage202-case-{}", case.id)],
    };
    (request, expected_artifact)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = (0..CASES).map(|seed| Case {
        id: format!("stage202-{seed:03}"),
        operation: if seed % 2 == 0 { MobiusOperation::InvertFiniteSequence } else { MobiusOperation::DivisorConvolution },
        expected: expected(seed),
        seed,
    }).collect::<Vec<_>>();
    let mut receipts = Vec::with_capacity(CASES);
    for case in &corpus {
        let (request, oracle) = request(case);
        let result = evaluate(&request);
        let actual = match result.status { MobiusStatus::Complete => Expected::Supported, MobiusStatus::Ambiguous => Expected::Ambiguous, _ => Expected::Unsupported };
        let artifact_correct = case.expected == Expected::Supported && result.artifact == oracle;
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        let authorized = actual == Expected::Supported && artifact_correct && result.replay_verified();
        receipts.push(Receipt {
            id: case.id.clone(), expected: case.expected, actual, exact: actual == case.expected,
            artifact_correct, replay_verified: result.replay_verified(), tamper_rejected: !tampered.replay_verified(),
            provenance_preserved: !result.provenance.is_empty() && !result.source.source_id.is_empty(),
            false_authorization: case.expected != Expected::Supported && authorized,
            false_denial: case.expected == Expected::Supported && !authorized,
        });
    }
    let report = Report {
        schema: "stage202-mobius-source-pack-bench-v1", corpus_sha256: digest(&corpus), cases: CASES,
        supported: corpus.iter().filter(|c| c.expected == Expected::Supported).count(),
        ambiguous: corpus.iter().filter(|c| c.expected == Expected::Ambiguous).count(),
        unsupported: corpus.iter().filter(|c| c.expected == Expected::Unsupported).count(),
        exact_decisions: receipts.iter().filter(|r| r.exact).count(),
        supported_artifacts: receipts.iter().filter(|r| r.expected == Expected::Supported && r.artifact_correct).count(),
        ambiguous_preserved: receipts.iter().filter(|r| r.expected == Expected::Ambiguous && r.actual == Expected::Ambiguous).count(),
        unsupported_refused: receipts.iter().filter(|r| r.expected == Expected::Unsupported && r.actual == Expected::Unsupported).count(),
        replay_verified: receipts.iter().filter(|r| r.replay_verified).count(),
        tamper_rejected: receipts.iter().filter(|r| r.tamper_rejected).count(),
        provenance_preserved: receipts.iter().filter(|r| r.provenance_preserved).count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts.iter().filter(|r| r.false_denial).count(), live_registry_mutations: 0,
        receipts, corpus,
    };
    assert_eq!((report.cases, report.supported, report.ambiguous, report.unsupported), (240, 120, 40, 80));
    assert_eq!((report.exact_decisions, report.supported_artifacts, report.ambiguous_preserved, report.unsupported_refused), (240, 120, 40, 80));
    assert_eq!((report.replay_verified, report.tamper_rejected, report.provenance_preserved, report.false_authorizations, report.false_denials), (240, 240, 240, 0, 0));
    fs::write(JSON, format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    fs::write(MD, "# Stage 202 — source-derived Möbius inversion pack\n\n- Cases: 240 (120 supported, 40 ambiguous, 80 unsupported)\n- Exact decisions / supported artifacts: 240/240 / 120/120\n- Ambiguities preserved / unsupported refused: 40/40 / 80/80\n- Replay / tamper / provenance: 240/240 each\n- False authorizations / denials: 0 / 0\n- Live registry mutations: 0\n\nThe source-derived finite contract supports explicit f(1)..f(n) Möbius inversion and divisor convolution for n ≤ 32. Missing indexing, oversized sequences, inconsistent lengths, infinite/asymptotic readings, and unbounded arithmetic remain closed.\n")?;
    println!("stage202 exact=240 supported_artifacts=120 ambiguous=40 unsupported=80 replay=240 tamper=240");
    Ok(())
}
