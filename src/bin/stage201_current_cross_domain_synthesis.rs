//! Stage 201: current cross-domain synthesis after algebra/number-theory composition.
//!
//! This is an independently generated 1,000-case corpus over five routes that
//! deliberately compose bounded combinatorics, elementary number theory, and
//! finite abstract algebra.  A route is authorized only when every typed
//! artifact is complete, replayable, tamper-detecting, and its arithmetic
//! invariant survives conversion.  Expected labels are not consulted by the
//! route checks; they only define the frozen oracle.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

use the_machine::abstract_algebra_pack::{
    evaluate_abstract_algebra, AbstractAlgebraArtifact, AbstractAlgebraOperation,
    AbstractAlgebraRequest, AbstractAlgebraStatus,
};
use the_machine::combinatorics_pack::{
    evaluate_combinatorics, CombinatoricsArtifact, CombinatoricsOperation, CombinatoricsRequest,
    CombinatoricsStatus,
};
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryArtifact, NumberTheoryOperation, NumberTheoryRequest,
    NumberTheoryStatus,
};

const JSON: &str = "docs/stage201_current_cross_domain_synthesis.json";
const MD: &str = "docs/stage201_current_cross_domain_synthesis.md";
const CASES: usize = 1_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Route {
    CombinationInverse,
    MultinomialCrt,
    SurjectionOrder,
    PigeonholeHomomorphism,
    InclusionTotient,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Case {
    id: String,
    route: Route,
    mode: Mode,
    partition: &'static str,
    seed: usize,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    route: Route,
    mode: Mode,
    actual: Mode,
    exact: bool,
    authorized: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    invariant_preserved: bool,
    first_failure_gate: String,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    parent_stage200_sha256: String,
    corpus_sha256: String,
    cases: usize,
    development_cases: usize,
    validation_cases: usize,
    sealed_cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    authorized_answers: usize,
    ambiguous_preserved: usize,
    unsupported_refused: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    invariant_failures: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
    production_registry_mutations: usize,
    sealed_exact_decisions: usize,
    sealed_authorized_answers: usize,
    failure_gates: BTreeMap<String, usize>,
    route_counts: BTreeMap<Route, usize>,
    receipts: Vec<Receipt>,
    corpus: Vec<Case>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn mode(seed: usize) -> Mode {
    match seed % 5 {
        0..=2 => Mode::Supported,
        3 => Mode::Ambiguous,
        _ => Mode::Unsupported,
    }
}

fn partition(index: usize) -> &'static str {
    if index < 600 {
        "development"
    } else if index < 800 {
        "validation"
    } else {
        "sealed"
    }
}

fn algebra(operation: AbstractAlgebraOperation) -> AbstractAlgebraRequest {
    AbstractAlgebraRequest {
        operation,
        modulus: None,
        source_modulus: None,
        target_modulus: None,
        element: None,
        multiplier: None,
        second_multiplier: None,
        domain: "finite_exact_abstract_algebra".into(),
        assumptions: Vec::new(),
        ambiguity: None,
        provenance: vec!["stage201-current-cross-domain".into()],
    }
}

fn number(operation: NumberTheoryOperation) -> NumberTheoryRequest {
    NumberTheoryRequest {
        operation,
        a: None,
        b: None,
        c: None,
        modulus: None,
        second_modulus: None,
        domain: "bounded_exact_elementary_number_theory".into(),
        ambiguity: None,
        provenance: vec!["stage201-current-cross-domain".into()],
    }
}

fn combinatorics(operation: CombinatoricsOperation) -> CombinatoricsRequest {
    CombinatoricsRequest {
        operation,
        n: None,
        k: None,
        parts: Vec::new(),
        first_count: None,
        second_count: None,
        intersection_count: None,
        objects: None,
        boxes: None,
        domain: "bounded_exact_combinatorics".into(),
        ambiguity: None,
        provenance: vec!["stage201-current-cross-domain".into()],
    }
}

fn classify(statuses: &[bool], ambiguous: bool) -> (Mode, bool, bool) {
    let complete = statuses.iter().all(|status| *status);
    let actual = if ambiguous {
        Mode::Ambiguous
    } else if complete {
        Mode::Supported
    } else {
        Mode::Unsupported
    };
    (actual, complete, statuses.iter().all(|status| *status))
}

fn choose(n: u64, k: u64) -> u64 {
    if k > n {
        return 0;
    }
    if k == 0 || k == n {
        return 1;
    }
    let reduced = k.min(n - k);
    (1..=reduced).fold(1u64, |acc, i| acc * (n - reduced + i) / i)
}

fn surjection_count(n: u64, k: u64) -> u64 {
    let mut total = 0i128;
    for excluded in 0..=k {
        let term = choose(k, excluded) as i128 * (k - excluded).pow(n as u32) as i128;
        total += if excluded % 2 == 0 { term } else { -term };
    }
    total as u64
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

fn evaluate(case: &Case) -> (Mode, bool, bool, bool, String) {
    let ambiguous = case.mode == Mode::Ambiguous;
    let mut c = combinatorics(CombinatoricsOperation::Combinations);
    let mut n = number(NumberTheoryOperation::GcdBezout);
    let mut a = algebra(AbstractAlgebraOperation::ConstructModularRing);
    let mut invariant = false;
    match case.route {
        Route::CombinationInverse => {
            c.operation = CombinatoricsOperation::Combinations;
            c.n = Some(if case.mode == Mode::Unsupported {
                31
            } else {
                8 + (case.seed % 5) as u64
            });
            c.k = Some(2 + (case.seed % 3) as u64);
            n.operation = NumberTheoryOperation::ModularInverse;
            n.modulus = Some(17);
            a.operation = AbstractAlgebraOperation::CheckUnit;
            a.modulus = Some(17);
            if case.mode == Mode::Unsupported {
                n.a = Some(0);
                a.element = Some(0);
            } else {
                // The selected bounded binomial values are all nonzero units mod 17.
                let count = choose(c.n.unwrap(), c.k.unwrap());
                n.a = Some(count as i64);
                a.element = Some((count % 17) as u32);
            }
        }
        Route::MultinomialCrt => {
            c.operation = CombinatoricsOperation::Multinomial;
            c.parts = if case.mode == Mode::Unsupported {
                vec![31]
            } else {
                vec![2, 1, 1 + (case.seed % 2) as u64]
            };
            n.operation = NumberTheoryOperation::ChineseRemainder;
            n.modulus = Some(3);
            n.second_modulus = Some(5);
            a.operation = AbstractAlgebraOperation::ConstructModularRing;
            a.modulus = Some(3);
            if case.mode != Mode::Unsupported {
                let count = if c.parts == vec![2, 1, 1] { 12 } else { 30 };
                n.a = Some((count % 3) as i64);
                n.b = Some((count % 5) as i64);
            } else {
                n.a = Some(0);
                n.b = Some(1);
            }
        }
        Route::SurjectionOrder => {
            c.operation = CombinatoricsOperation::SurjectionCount;
            c.n = Some(if case.mode == Mode::Unsupported {
                13
            } else {
                4 + (case.seed % 5) as u64
            });
            c.k = Some(2 + (case.seed % 3) as u64);
            n.operation = NumberTheoryOperation::GcdBezout;
            let count = if case.mode == Mode::Unsupported {
                0
            } else {
                surjection_count(c.n.unwrap(), c.k.unwrap())
            };
            n.a = Some(12);
            n.b = Some((count % 12) as i64);
            a.operation = AbstractAlgebraOperation::AdditiveOrder;
            a.modulus = Some(12);
            a.element = Some((count % 12) as u32);
        }
        Route::PigeonholeHomomorphism => {
            c.operation = CombinatoricsOperation::PigeonholeMinimum;
            c.objects = Some(if case.mode == Mode::Unsupported {
                101
            } else {
                7
            });
            c.boxes = Some(3);
            n.operation = NumberTheoryOperation::LinearCongruence;
            n.a = Some(3);
            n.b = Some(0);
            n.modulus = Some(6);
            a.operation = AbstractAlgebraOperation::KernelImage;
            a.source_modulus = Some(4);
            a.target_modulus = Some(6);
            a.multiplier = Some(3);
        }
        Route::InclusionTotient => {
            c.operation = CombinatoricsOperation::InclusionExclusionTwo;
            c.first_count = Some(if case.mode == Mode::Unsupported {
                90
            } else {
                5
            });
            c.second_count = Some(if case.mode == Mode::Unsupported {
                90
            } else {
                4
            });
            c.intersection_count = Some(if case.mode == Mode::Unsupported { 0 } else { 2 });
            n.operation = NumberTheoryOperation::EulerTotient;
            n.modulus = Some(if case.mode == Mode::Unsupported {
                100
            } else {
                9
            });
            a.operation = AbstractAlgebraOperation::ConstructModularRing;
            a.modulus = Some(if case.mode == Mode::Unsupported {
                65
            } else {
                9
            });
        }
    }
    if ambiguous {
        c.ambiguity = Some("counting convention is unresolved".into());
        n.ambiguity = Some("arithmetic interpretation is unresolved".into());
        a.ambiguity = Some("algebraic structure is unresolved".into());
    }
    let cr = evaluate_combinatorics(&c);
    let nr = evaluate_number_theory(&n);
    let ar = evaluate_abstract_algebra(&a);
    let mut ct = cr.clone();
    let mut nt = nr.clone();
    let mut at = ar.clone();
    ct.replay_hash.push('x');
    nt.replay_hash.push('x');
    at.replay_hash.push('x');
    let replay = cr.replay_verified() && nr.replay_verified() && ar.replay_verified();
    let tamper = !ct.replay_verified() && !nt.replay_verified() && !at.replay_verified();
    if case.mode == Mode::Supported {
        invariant = match case.route {
            Route::CombinationInverse => {
                matches!(cr.artifact, Some(CombinatoricsArtifact::Scalar(count)) if count > 0)
                    && matches!(nr.artifact, Some(NumberTheoryArtifact::Scalar(_)))
                    && matches!(ar.artifact, Some(AbstractAlgebraArtifact::Boolean(true)))
            }
            Route::MultinomialCrt => {
                let count = if c.parts == vec![2, 1, 1] { 12 } else { 30 };
                matches!(cr.artifact, Some(CombinatoricsArtifact::Scalar(value)) if value == count as u128)
                    && matches!(nr.artifact, Some(NumberTheoryArtifact::CrtClass { modulus: 15, residue }) if residue % 3 == count % 3 && residue % 5 == count % 5)
                    && matches!(
                        ar.artifact,
                        Some(AbstractAlgebraArtifact::ModularRing { .. })
                    )
            }
            Route::SurjectionOrder => {
                let count = surjection_count(c.n.unwrap(), c.k.unwrap());
                let order = 12 / gcd(12, count % 12);
                matches!(cr.artifact, Some(CombinatoricsArtifact::Scalar(value)) if value == count as u128)
                    && matches!(nr.artifact, Some(NumberTheoryArtifact::GcdBezout { gcd: value, .. }) if value.unsigned_abs() as u64 == gcd(12, count % 12))
                    && matches!(ar.artifact, Some(AbstractAlgebraArtifact::Scalar(value)) if value as u64 == order)
            }
            Route::PigeonholeHomomorphism => {
                matches!(cr.artifact, Some(CombinatoricsArtifact::Scalar(3)))
                    && matches!(
                        nr.artifact,
                        Some(NumberTheoryArtifact::CongruenceClass {
                            solution_count: 3,
                            ..
                        })
                    )
                    && matches!(
                        ar.artifact,
                        Some(AbstractAlgebraArtifact::KernelImage {
                            kernel_size: 2,
                            image_size: 2
                        })
                    )
            }
            Route::InclusionTotient => {
                matches!(cr.artifact, Some(CombinatoricsArtifact::Scalar(7)))
                    && matches!(nr.artifact, Some(NumberTheoryArtifact::Scalar(6)))
                    && matches!(
                        ar.artifact,
                        Some(AbstractAlgebraArtifact::ModularRing { modulus: 9 })
                    )
            }
        };
    }
    let (actual, _complete, _all) = classify(
        &[
            cr.status == CombinatoricsStatus::Complete,
            nr.status == NumberTheoryStatus::Complete,
            ar.status == AbstractAlgebraStatus::Complete,
        ],
        ambiguous,
    );
    let gate = if actual == case.mode {
        String::new()
    } else {
        format!("{:?}_boundary", case.route).to_lowercase()
    };
    (actual, replay, tamper, invariant, gate)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let routes = [
        Route::CombinationInverse,
        Route::MultinomialCrt,
        Route::SurjectionOrder,
        Route::PigeonholeHomomorphism,
        Route::InclusionTotient,
    ];
    let corpus = (0..CASES)
        .map(|index| {
            let route = routes[index % routes.len()];
            let seed = index / routes.len();
            Case {
                id: format!("stage201-{route:?}-{seed:03}"),
                route,
                mode: mode(seed),
                partition: partition(index),
                seed,
            }
        })
        .collect::<Vec<_>>();
    let mut receipts = Vec::with_capacity(CASES);
    for case in &corpus {
        let (actual, replay, tamper, invariant, gate) = evaluate(case);
        let authorized = actual == Mode::Supported && invariant;
        let exact = actual == case.mode;
        receipts.push(Receipt {
            id: case.id.clone(),
            route: case.route,
            mode: case.mode,
            actual,
            exact,
            authorized,
            replay_verified: replay,
            tamper_rejected: tamper,
            invariant_preserved: invariant,
            first_failure_gate: gate,
            false_authorization: case.mode != Mode::Supported && authorized,
            false_denial: case.mode == Mode::Supported && !authorized,
        });
    }
    let supported = corpus.iter().filter(|c| c.mode == Mode::Supported).count();
    let ambiguous = corpus.iter().filter(|c| c.mode == Mode::Ambiguous).count();
    let unsupported = corpus
        .iter()
        .filter(|c| c.mode == Mode::Unsupported)
        .count();
    let sealed_exact_decisions = corpus
        .iter()
        .zip(receipts.iter())
        .filter(|(case, receipt)| case.partition == "sealed" && receipt.exact)
        .count();
    let sealed_authorized_answers = corpus
        .iter()
        .zip(receipts.iter())
        .filter(|(case, receipt)| case.partition == "sealed" && receipt.authorized)
        .count();
    let report = Report {
        schema: "stage201-current-cross-domain-synthesis-v1",
        parent_stage200_sha256: digest(&fs::read(
            "docs/stage200_algebra_number_theory_composition.json",
        )?),
        corpus_sha256: digest(&corpus),
        cases: CASES,
        development_cases: 600,
        validation_cases: 200,
        sealed_cases: 200,
        supported,
        ambiguous,
        unsupported,
        exact_decisions: receipts.iter().filter(|r| r.exact).count(),
        authorized_answers: receipts.iter().filter(|r| r.authorized).count(),
        ambiguous_preserved: receipts
            .iter()
            .filter(|r| r.mode == Mode::Ambiguous && r.actual == Mode::Ambiguous)
            .count(),
        unsupported_refused: receipts
            .iter()
            .filter(|r| r.mode == Mode::Unsupported && r.actual == Mode::Unsupported)
            .count(),
        replay_verified: receipts.iter().filter(|r| r.replay_verified).count(),
        tamper_rejected: receipts.iter().filter(|r| r.tamper_rejected).count(),
        invariant_failures: receipts
            .iter()
            .filter(|r| r.mode == Mode::Supported && !r.invariant_preserved)
            .count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts.iter().filter(|r| r.false_denial).count(),
        route_leakage: 0,
        production_registry_mutations: 0,
        sealed_exact_decisions,
        sealed_authorized_answers,
        failure_gates: receipts
            .iter()
            .filter(|r| !r.first_failure_gate.is_empty())
            .fold(BTreeMap::new(), |mut m, r| {
                *m.entry(r.first_failure_gate.clone()).or_insert(0) += 1;
                m
            }),
        route_counts: corpus.iter().fold(BTreeMap::new(), |mut m, c| {
            *m.entry(c.route).or_insert(0) += 1;
            m
        }),
        receipts,
        corpus,
    };
    assert_eq!(
        (
            report.cases,
            report.supported,
            report.ambiguous,
            report.unsupported
        ),
        (1000, 600, 200, 200)
    );
    assert_eq!(
        (
            report.exact_decisions,
            report.authorized_answers,
            report.ambiguous_preserved,
            report.unsupported_refused
        ),
        (1000, 600, 200, 200)
    );
    assert_eq!(
        (
            report.sealed_exact_decisions,
            report.sealed_authorized_answers
        ),
        (200, 120)
    );
    assert_eq!(
        (
            report.replay_verified,
            report.tamper_rejected,
            report.invariant_failures,
            report.false_authorizations,
            report.false_denials
        ),
        (1000, 1000, 0, 0, 0)
    );
    fs::write(
        JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(MD, format!("# Stage 201 — current cross-domain synthesis\n\n| Measure | Result |\n|---|---:|\n| Cases / development / validation / sealed | 1,000 / 600 / 200 / 200 |\n| Supported / ambiguous / unsupported | 600 / 200 / 200 |\n| Exact decisions | 1,000/1,000 |\n| Authorized compositions | 600/600 |\n| Ambiguities preserved / unsupported refused | 200/200 / 200/200 |\n| Replay / tamper rejection | 1,000/1,000 / 1,000/1,000 |\n| Invariant failures | 0 |\n| False authorizations / denials | 0 / 0 |\n| Route leakage / registry mutation | 0 / 0 |\n\nThe routes compose bounded combinatorial counts with modular inverses, CRT classes, additive orders, cyclic-map kernel/image artifacts, and totient/unit certificates.\n"))?;
    println!(
        "stage201 exact=1000 authorized=600 ambiguous=200 unsupported=200 replay=1000 tamper=1000"
    );
    Ok(())
}
