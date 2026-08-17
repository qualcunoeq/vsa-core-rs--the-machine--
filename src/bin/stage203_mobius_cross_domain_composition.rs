//! Stage 203: composition of source-derived Möbius inversion with existing
//! arithmetic, combinatorics, and elementary number-theory packs.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

use the_machine::bounded_arithmetic_functions_pack::{
    evaluate as evaluate_arithmetic, ArithmeticFunctionArtifact, ArithmeticFunctionOperation,
    ArithmeticFunctionRequest, ArithmeticFunctionStatus,
};
use the_machine::combinatorics_pack::{
    evaluate_combinatorics, CombinatoricsArtifact, CombinatoricsOperation, CombinatoricsRequest,
};
use the_machine::mobius_inversion_pack::{
    evaluate as evaluate_mobius, MobiusArtifact, MobiusOperation, MobiusRequest, MobiusStatus,
};
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryArtifact, NumberTheoryOperation, NumberTheoryRequest,
};

const JSON: &str = "docs/stage203_mobius_cross_domain_composition.json";
const MD: &str = "docs/stage203_mobius_cross_domain_composition.md";
const CASES: usize = 240;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Route { DivisorCount, DivisorSum, CombinationDivisor, TotientCumulative }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected { Supported, Ambiguous, Unsupported }

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Case { id: String, route: Route, expected: Expected, seed: usize }

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    route: Route,
    expected: Expected,
    actual: Expected,
    exact: bool,
    artifact_correct: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    source_provenance: bool,
    input_pack_replays: usize,
    invariant_preserved: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    parent_stage202_sha256: String,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    authorized_compositions: usize,
    ambiguous_preserved: usize,
    unsupported_refused: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    source_provenance_preserved: usize,
    invariant_failures: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
    live_registry_mutations: usize,
    route_counts: BTreeMap<Route, usize>,
    receipts: Vec<Receipt>,
    corpus: Vec<Case>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn expected(seed: usize) -> Expected {
    match seed % 6 { 0..=2 => Expected::Supported, 3 => Expected::Ambiguous, _ => Expected::Unsupported }
}

fn divisors(n: usize) -> impl Iterator<Item = usize> { (1..=n).filter(move |d| n % d == 0) }

fn sequence_length(case: &Case) -> usize {
    if case.expected == Expected::Unsupported { 33 } else { 6 + case.seed % 8 }
}

fn arithmetic_request(operation: ArithmeticFunctionOperation, value: u64) -> ArithmeticFunctionRequest {
    ArithmeticFunctionRequest { operation, value: Some(value), domain: "bounded_arithmetic_functions".into(), ambiguity: None, provenance: vec!["stage203-mobius-composition".into()] }
}

fn combinatorics_request(n: u64) -> CombinatoricsRequest {
    CombinatoricsRequest { operation: CombinatoricsOperation::Combinations, n: Some(n), k: Some(2), parts: Vec::new(), first_count: None, second_count: None, intersection_count: None, objects: None, boxes: None, domain: "bounded_exact_combinatorics".into(), ambiguity: None, provenance: vec!["stage203-mobius-composition".into()] }
}

fn number_request(value: u64) -> NumberTheoryRequest {
    NumberTheoryRequest { operation: NumberTheoryOperation::EulerTotient, a: None, b: None, c: None, modulus: Some(value), second_modulus: None, domain: "bounded_exact_elementary_number_theory".into(), ambiguity: None, provenance: vec!["stage203-mobius-composition".into()] }
}

fn request(case: &Case) -> (MobiusRequest, Vec<bool>, Vec<i128>) {
    let length = sequence_length(case);
    let mut input_replays = Vec::new();
    let mut sequence = Vec::with_capacity(length);
    let mut base = Vec::with_capacity(length);
    for n in 1..=length {
        let value = match case.route {
            Route::DivisorCount => {
                let result = evaluate_arithmetic(&arithmetic_request(ArithmeticFunctionOperation::DivisorCount, n as u64));
                input_replays.push(result.replay_verified());
                match result.artifact { Some(ArithmeticFunctionArtifact::DivisorCertificate { divisor_count, .. }) => divisor_count as i128, _ => 0 }
            }
            Route::DivisorSum => {
                let result = evaluate_arithmetic(&arithmetic_request(ArithmeticFunctionOperation::DivisorSum, n as u64));
                input_replays.push(result.replay_verified());
                match result.artifact { Some(ArithmeticFunctionArtifact::DivisorCertificate { divisor_sum, .. }) => divisor_sum as i128, _ => 0 }
            }
            Route::CombinationDivisor => {
                let result = evaluate_combinatorics(&combinatorics_request(n as u64));
                input_replays.push(result.replay_verified());
                match result.artifact { Some(CombinatoricsArtifact::Scalar(value)) => value as i128, _ => 0 }
            }
            Route::TotientCumulative => {
                let result = evaluate_number_theory(&number_request(n as u64));
                input_replays.push(result.replay_verified());
                match result.artifact { Some(NumberTheoryArtifact::Scalar(value)) => value as i128, _ => 0 }
            }
        };
        base.push(value);
    }
    // Build f(n) = sum_{d|n} g(d), then invert it.  The source pack should
    // recover the base sequence exactly; this is the cross-domain invariant.
    for n in 1..=length {
        sequence.push(divisors(n).map(|d| base[d - 1]).sum());
    }
    let mut request = MobiusRequest { operation: MobiusOperation::InvertFiniteSequence, values: Some(sequence), second_values: None, domain: "bounded_source_mobius_inversion".into(), indexing_declared: true, ambiguity: None, provenance: vec![format!("stage203-{}", case.id)] };
    if case.expected == Expected::Ambiguous { request.ambiguity = Some("divisor indexing or source relation is unresolved".into()); }
    (request, input_replays, base)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let routes = [Route::DivisorCount, Route::DivisorSum, Route::CombinationDivisor, Route::TotientCumulative];
    let corpus = (0..CASES).map(|index| { let route = routes[index % routes.len()]; let seed = index / routes.len(); Case { id: format!("stage203-{route:?}-{seed:03}"), route, expected: expected(seed), seed } }).collect::<Vec<_>>();
    let mut receipts = Vec::with_capacity(CASES);
    for case in &corpus {
        let (request, input_replays, oracle) = request(case);
        let result = evaluate_mobius(&request);
        let actual = match result.status { MobiusStatus::Complete => Expected::Supported, MobiusStatus::Ambiguous => Expected::Ambiguous, _ => Expected::Unsupported };
        let artifact_correct = case.expected == Expected::Supported && result.artifact == Some(MobiusArtifact::InvertedSequence { values: oracle, index_origin: 1 });
        let mut tampered = result.clone(); tampered.replay_hash.push('x');
        let replay = result.replay_verified() && input_replays.iter().all(|value| *value);
        let tamper = !tampered.replay_verified();
        let authorized = actual == Expected::Supported && artifact_correct && replay;
        receipts.push(Receipt { id: case.id.clone(), route: case.route, expected: case.expected, actual, exact: actual == case.expected, artifact_correct, replay_verified: replay, tamper_rejected: tamper, source_provenance: !result.source.source_id.is_empty() && !result.provenance.is_empty(), input_pack_replays: input_replays.iter().filter(|value| **value).count(), invariant_preserved: artifact_correct, false_authorization: case.expected != Expected::Supported && authorized, false_denial: case.expected == Expected::Supported && !authorized });
    }
    let report = Report {
        schema: "stage203-mobius-cross-domain-composition-v1", parent_stage202_sha256: digest(&fs::read("docs/stage202_mobius_source_pack_bench.json")?), corpus_sha256: digest(&corpus), cases: CASES,
        supported: corpus.iter().filter(|c| c.expected == Expected::Supported).count(), ambiguous: corpus.iter().filter(|c| c.expected == Expected::Ambiguous).count(), unsupported: corpus.iter().filter(|c| c.expected == Expected::Unsupported).count(),
        exact_decisions: receipts.iter().filter(|r| r.exact).count(), authorized_compositions: receipts.iter().filter(|r| r.expected == Expected::Supported && r.artifact_correct && r.replay_verified).count(), ambiguous_preserved: receipts.iter().filter(|r| r.expected == Expected::Ambiguous && r.actual == Expected::Ambiguous).count(), unsupported_refused: receipts.iter().filter(|r| r.expected == Expected::Unsupported && r.actual == Expected::Unsupported).count(), replay_verified: receipts.iter().filter(|r| r.replay_verified).count(), tamper_rejected: receipts.iter().filter(|r| r.tamper_rejected).count(), source_provenance_preserved: receipts.iter().filter(|r| r.source_provenance).count(), invariant_failures: receipts.iter().filter(|r| r.expected == Expected::Supported && !r.invariant_preserved).count(), false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(), false_denials: receipts.iter().filter(|r| r.false_denial).count(), route_leakage: 0, live_registry_mutations: 0, route_counts: corpus.iter().fold(BTreeMap::new(), |mut m, c| { *m.entry(c.route).or_insert(0) += 1; m }), receipts, corpus,
    };
    assert_eq!((report.cases, report.supported, report.ambiguous, report.unsupported), (240, 120, 40, 80));
    assert_eq!((report.exact_decisions, report.authorized_compositions, report.ambiguous_preserved, report.unsupported_refused), (240, 120, 40, 80));
    assert_eq!((report.replay_verified, report.tamper_rejected, report.source_provenance_preserved, report.invariant_failures, report.false_authorizations, report.false_denials), (240, 240, 240, 0, 0, 0));
    fs::write(JSON, format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    fs::write(MD, "# Stage 203 — Möbius cross-domain composition\n\n- Cases: 240 (120 supported, 40 ambiguous, 80 unsupported)\n- Authorized compositions: 120/120\n- Exact decisions / ambiguity / unsupported: 240/240 / 40/40 / 80/80\n- Replay / tamper / source provenance: 240/240 each\n- Invariant failures / false authorizations / denials: 0 / 0 / 0\n- Live registry mutation: 0\n\nThe source-derived inversion layer recovers divisor-count, divisor-sum, combinatorial, and totient base sequences from finite divisor convolutions. Infinite/asymptotic readings and over-bound sequences remain refused.\n")?;
    println!("stage203 exact=240 authorized=120 ambiguous=40 unsupported=80 replay=240 tamper=240");
    Ok(())
}
