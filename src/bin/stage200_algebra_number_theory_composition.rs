//! Stage 200: exact composition of the abstract-algebra and number-theory packs.
//!
//! This campaign checks that arithmetic conditions survive typed conversion:
//! units remain coprime, congruence classes retain their solution count, CRT
//! classes retain compatibility, and cyclic-map kernel/image invariants agree
//! with the corresponding bounded congruence calculation.  It deliberately
//! exercises both packs independently; no route is authorized from a label or
//! from an expected outcome.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

use the_machine::abstract_algebra_pack::{
    evaluate_abstract_algebra, AbstractAlgebraArtifact, AbstractAlgebraOperation,
    AbstractAlgebraRequest, AbstractAlgebraStatus,
};
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryArtifact, NumberTheoryOperation, NumberTheoryRequest,
    NumberTheoryStatus,
};

const REPORT_JSON: &str = "docs/stage200_algebra_number_theory_composition.json";
const REPORT_MD: &str = "docs/stage200_algebra_number_theory_composition.md";
const CASES_PER_ROUTE: usize = 40;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Route {
    BezoutInverse,
    CongruenceUnit,
    ChineseRemainderRing,
    HomomorphismKernel,
    AdditiveOrderGcd,
    TotientUnitCount,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Case {
    id: String,
    route: Route,
    expected: Expected,
    index: usize,
}

#[derive(Debug, Clone, Serialize)]
struct Receipt {
    id: String,
    route: Route,
    expected: Expected,
    actual: Expected,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    route_agreed: bool,
    invariants_preserved: bool,
    false_authorization: bool,
    false_denial: bool,
    first_failure_gate: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    parent_report_sha256: String,
    corpus_sha256: String,
    cases: usize,
    supported_cases: usize,
    ambiguous_cases: usize,
    unsupported_cases: usize,
    exact_decisions: usize,
    authorized_compositions: usize,
    ambiguous_preserved: usize,
    unsupported_refused: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    route_disagreements: usize,
    invariant_failures: usize,
    false_authorizations: usize,
    false_denials: usize,
    production_registry_mutations: usize,
    route_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
    corpus: Vec<Case>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn expected(local: usize, route: Route) -> Expected {
    match route {
        Route::BezoutInverse
        | Route::CongruenceUnit
        | Route::ChineseRemainderRing
        | Route::HomomorphismKernel => match local {
            0..=19 => Expected::Supported,
            20..=26 => Expected::Ambiguous,
            _ => Expected::Unsupported,
        },
        Route::AdditiveOrderGcd | Route::TotientUnitCount => match local {
            0..=19 => Expected::Supported,
            20..=25 => Expected::Ambiguous,
            _ => Expected::Unsupported,
        },
    }
}

fn route_name(route: Route) -> String {
    format!("{route:?}").to_lowercase()
}

fn algebra_request(operation: AbstractAlgebraOperation) -> AbstractAlgebraRequest {
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
        provenance: vec!["stage200-algebra-number-theory".into()],
    }
}

fn number_request(operation: NumberTheoryOperation) -> NumberTheoryRequest {
    NumberTheoryRequest {
        operation,
        a: None,
        b: None,
        c: None,
        modulus: None,
        second_modulus: None,
        domain: "bounded_exact_elementary_number_theory".into(),
        ambiguity: None,
        provenance: vec!["stage200-algebra-number-theory".into()],
    }
}

fn actual(status_a: AbstractAlgebraStatus, status_n: NumberTheoryStatus) -> Expected {
    if status_a == AbstractAlgebraStatus::Ambiguous || status_n == NumberTheoryStatus::Ambiguous {
        Expected::Ambiguous
    } else if status_a == AbstractAlgebraStatus::Complete
        && status_n == NumberTheoryStatus::Complete
    {
        Expected::Supported
    } else {
        Expected::Unsupported
    }
}

fn evaluate(case: &Case) -> (Expected, bool, bool, bool, String) {
    let mut algebra = algebra_request(AbstractAlgebraOperation::ConstructModularRing);
    let mut number = number_request(NumberTheoryOperation::GcdBezout);
    let mut invariant = false;
    match case.route {
        Route::BezoutInverse => {}
        Route::CongruenceUnit => {
            algebra.operation = AbstractAlgebraOperation::CheckUnit;
            algebra.modulus = Some(if case.expected == Expected::Unsupported {
                12
            } else {
                13
            });
            algebra.element = Some(if case.expected == Expected::Unsupported {
                6
            } else {
                5
            });
            number.operation = NumberTheoryOperation::LinearCongruence;
            number.a = Some(if case.expected == Expected::Unsupported {
                6
            } else {
                5
            });
            number.b = Some(if case.expected == Expected::Unsupported {
                1
            } else {
                (case.index % 13) as i64
            });
            number.modulus = Some(if case.expected == Expected::Unsupported {
                12
            } else {
                13
            });
        }
        Route::ChineseRemainderRing => {
            algebra.operation = AbstractAlgebraOperation::ConstructModularRing;
            algebra.modulus = Some(if case.expected == Expected::Unsupported {
                1
            } else {
                3
            });
            number.operation = NumberTheoryOperation::ChineseRemainder;
            number.a = Some(if case.expected == Expected::Unsupported {
                0
            } else {
                (case.index % 3) as i64
            });
            number.b = Some(if case.expected == Expected::Unsupported {
                1
            } else {
                (case.index * 2 + 1) as i64
            });
            number.modulus = Some(if case.expected == Expected::Unsupported {
                4
            } else {
                3
            });
            number.second_modulus = Some(if case.expected == Expected::Unsupported {
                6
            } else {
                5
            });
        }
        Route::HomomorphismKernel => {
            algebra.operation = AbstractAlgebraOperation::KernelImage;
            algebra.source_modulus = Some(4);
            algebra.target_modulus = Some(6);
            algebra.multiplier = Some(if case.expected == Expected::Unsupported {
                1
            } else {
                3
            });
            number.operation = NumberTheoryOperation::LinearCongruence;
            number.a = Some(if case.expected == Expected::Unsupported {
                1
            } else {
                3
            });
            number.b = Some(0);
            number.modulus = Some(6);
        }
        Route::AdditiveOrderGcd => {
            algebra.operation = AbstractAlgebraOperation::AdditiveOrder;
            algebra.modulus = Some(if case.expected == Expected::Unsupported {
                0
            } else {
                12
            });
            algebra.element = Some((case.index % 11 + 1) as u32);
            number.operation = NumberTheoryOperation::GcdBezout;
            number.a = Some(12);
            number.b = Some((case.index % 11 + 1) as i64);
        }
        Route::TotientUnitCount => {
            algebra.operation = AbstractAlgebraOperation::ConstructModularRing;
            algebra.modulus = Some(if case.expected == Expected::Unsupported {
                1
            } else {
                (2 + case.index % 20) as u32
            });
            number.operation = NumberTheoryOperation::EulerTotient;
            number.modulus = Some(if case.expected == Expected::Unsupported {
                1
            } else {
                (2 + case.index % 20) as u64
            });
        }
    }
    // The first route is spelled out after the shared initialization to keep
    // the request fields visible in the receipt-level audit.
    if case.route == Route::BezoutInverse {
        let pairs = [
            (3, 10),
            (5, 12),
            (7, 15),
            (5, 14),
            (9, 16),
            (11, 18),
            (7, 20),
            (13, 22),
            (5, 18),
            (17, 24),
        ];
        let (a, m) = pairs[case.index % pairs.len()];
        algebra.operation = AbstractAlgebraOperation::CheckUnit;
        algebra.modulus = Some(if case.expected == Expected::Unsupported {
            12
        } else {
            m as u32
        });
        algebra.element = Some(if case.expected == Expected::Unsupported {
            6
        } else {
            a as u32
        });
        number.operation = if case.expected == Expected::Unsupported {
            NumberTheoryOperation::ModularInverse
        } else {
            NumberTheoryOperation::GcdBezout
        };
        number.a = Some(if case.expected == Expected::Unsupported {
            6
        } else {
            a
        });
        number.b = Some(m);
        number.modulus = Some(m as u64);
        if case.expected == Expected::Supported {
            let gcd = evaluate_number_theory(&number);
            let inverse = evaluate_number_theory(&NumberTheoryRequest {
                operation: NumberTheoryOperation::ModularInverse,
                a: Some(a),
                b: None,
                c: None,
                modulus: Some(m as u64),
                second_modulus: None,
                domain: number.domain.clone(),
                ambiguity: None,
                provenance: number.provenance.clone(),
            });
            invariant = matches!(
                gcd.artifact,
                Some(NumberTheoryArtifact::GcdBezout { gcd: 1, .. })
            ) && inverse.status == NumberTheoryStatus::Complete;
            let (a_result, n_result) = (
                evaluate_abstract_algebra(&algebra),
                evaluate_number_theory(&number),
            );
            let actual = actual(a_result.status, n_result.status);
            let mut algebra_tampered = a_result.clone();
            algebra_tampered.replay_hash.push('x');
            let mut number_tampered = n_result.clone();
            number_tampered.replay_hash.push('x');
            let mut inverse_tampered = inverse.clone();
            inverse_tampered.replay_hash.push('x');
            let replay = a_result.replay_verified()
                && n_result.replay_verified()
                && inverse.replay_verified();
            let tamper = !algebra_tampered.replay_verified()
                && !number_tampered.replay_verified()
                && !inverse_tampered.replay_verified();
            return (
                actual,
                replay,
                tamper,
                invariant,
                "bezout_to_inverse".into(),
            );
        }
    }
    if case.expected == Expected::Ambiguous {
        algebra.ambiguity = Some("arithmetic structure or notation is unresolved".into());
        number.ambiguity = Some("arithmetic structure or notation is unresolved".into());
    }
    let a_result = evaluate_abstract_algebra(&algebra);
    let n_result = evaluate_number_theory(&number);
    let actual = actual(a_result.status, n_result.status);
    let mut algebra_tampered = a_result.clone();
    algebra_tampered.replay_hash.push('x');
    let mut number_tampered = n_result.clone();
    number_tampered.replay_hash.push('x');
    let replay = a_result.replay_verified() && n_result.replay_verified();
    let tamper = !algebra_tampered.replay_verified() && !number_tampered.replay_verified();
    match case.route {
        Route::CongruenceUnit => {
            invariant = matches!(
                a_result.artifact,
                Some(AbstractAlgebraArtifact::Boolean(true))
            ) && matches!(
                n_result.artifact,
                Some(NumberTheoryArtifact::CongruenceClass {
                    solution_count: 1,
                    ..
                })
            );
        }
        Route::ChineseRemainderRing => {
            invariant = matches!(
                a_result.artifact,
                Some(AbstractAlgebraArtifact::ModularRing { .. })
            ) && matches!(
                n_result.artifact,
                Some(NumberTheoryArtifact::CrtClass { .. })
            );
        }
        Route::HomomorphismKernel => {
            invariant = matches!(
                a_result.artifact,
                Some(AbstractAlgebraArtifact::KernelImage {
                    kernel_size: 2,
                    image_size: 2
                })
            ) && matches!(
                n_result.artifact,
                Some(NumberTheoryArtifact::CongruenceClass {
                    solution_count: 3,
                    ..
                })
            );
        }
        Route::AdditiveOrderGcd => {
            let m = 12u64;
            let e = (case.index % 11 + 1) as u64;
            let expected_order = m / gcd_u64(m, e);
            invariant = matches!(a_result.artifact, Some(AbstractAlgebraArtifact::Scalar(value)) if value as u64 == expected_order)
                && matches!(n_result.artifact, Some(NumberTheoryArtifact::GcdBezout { gcd, .. }) if gcd.unsigned_abs() as u64 == gcd_u64(m, e));
        }
        Route::TotientUnitCount => {
            if let (
                Some(AbstractAlgebraArtifact::ModularRing { modulus }),
                Some(NumberTheoryArtifact::Scalar(phi)),
            ) = (a_result.artifact.as_ref(), n_result.artifact.as_ref())
            {
                let unit_count = (0..*modulus)
                    .filter(|element| gcd_u64(*modulus as u64, *element as u64) == 1)
                    .count() as u64;
                invariant = unit_count == *phi;
            }
        }
        Route::BezoutInverse => {}
    }
    (
        actual,
        replay,
        tamper,
        invariant,
        format!("{}", route_name(case.route)),
    )
}

fn gcd_u64(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

fn build_corpus() -> Vec<Case> {
    let routes = [
        Route::BezoutInverse,
        Route::CongruenceUnit,
        Route::ChineseRemainderRing,
        Route::HomomorphismKernel,
        Route::AdditiveOrderGcd,
        Route::TotientUnitCount,
    ];
    routes
        .into_iter()
        .flat_map(|route| {
            (0..CASES_PER_ROUTE).map(move |index| Case {
                id: format!("stage200-{}-{index:03}", route_name(route)),
                route,
                expected: expected(index, route),
                index,
            })
        })
        .collect()
}

fn main() {
    let corpus = build_corpus();
    let mut receipts = Vec::with_capacity(corpus.len());
    for case in &corpus {
        let (actual, replay, tamper, invariant, gate) = evaluate(case);
        let exact = actual == case.expected;
        let authorized = actual == Expected::Supported && invariant;
        let false_authorization =
            case.expected != Expected::Supported && actual == Expected::Supported;
        let false_denial = case.expected == Expected::Supported && !authorized;
        let tamper_rejected = tamper;
        receipts.push(Receipt {
            id: case.id.clone(),
            route: case.route,
            expected: case.expected,
            actual,
            exact,
            replay_verified: replay,
            tamper_rejected,
            route_agreed: exact,
            invariants_preserved: invariant,
            false_authorization,
            false_denial,
            first_failure_gate: if exact { "none".into() } else { gate },
        });
    }
    let supported_cases = corpus
        .iter()
        .filter(|c| c.expected == Expected::Supported)
        .count();
    let ambiguous_cases = corpus
        .iter()
        .filter(|c| c.expected == Expected::Ambiguous)
        .count();
    let unsupported_cases = corpus
        .iter()
        .filter(|c| c.expected == Expected::Unsupported)
        .count();
    let report = Report {
        schema: "stage200-algebra-number-theory-composition-v1",
        parent_report_sha256: digest(
            &fs::read("docs/stage199_current_integrated_checkpoint.json")
                .expect("parent checkpoint"),
        ),
        corpus_sha256: digest(&corpus),
        cases: corpus.len(),
        supported_cases,
        ambiguous_cases,
        unsupported_cases,
        exact_decisions: receipts.iter().filter(|r| r.exact).count(),
        authorized_compositions: receipts
            .iter()
            .filter(|r| {
                r.expected == Expected::Supported
                    && r.actual == Expected::Supported
                    && r.invariants_preserved
            })
            .count(),
        ambiguous_preserved: receipts
            .iter()
            .filter(|r| r.expected == Expected::Ambiguous && r.actual == Expected::Ambiguous)
            .count(),
        unsupported_refused: receipts
            .iter()
            .filter(|r| r.expected == Expected::Unsupported && r.actual == Expected::Unsupported)
            .count(),
        replay_verified: receipts.iter().filter(|r| r.replay_verified).count(),
        tamper_rejected: receipts.iter().filter(|r| r.tamper_rejected).count(),
        route_disagreements: receipts.iter().filter(|r| !r.route_agreed).count(),
        invariant_failures: receipts
            .iter()
            .filter(|r| !r.invariants_preserved && r.expected == Expected::Supported)
            .count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts.iter().filter(|r| r.false_denial).count(),
        production_registry_mutations: 0,
        route_counts: corpus.iter().fold(BTreeMap::new(), |mut map, case| {
            *map.entry(route_name(case.route)).or_insert(0) += 1;
            map
        }),
        receipts,
        corpus,
    };
    assert_eq!(report.cases, 240);
    assert_eq!(report.supported_cases, 120);
    assert_eq!(report.ambiguous_cases, 40);
    assert_eq!(report.unsupported_cases, 80);
    assert_eq!(report.exact_decisions, 240);
    assert_eq!(report.authorized_compositions, 120);
    assert_eq!(report.ambiguous_preserved, 40);
    assert_eq!(report.unsupported_refused, 80);
    assert_eq!(report.replay_verified, 240);
    assert_eq!(report.tamper_rejected, 240);
    assert_eq!(report.route_disagreements, 0);
    assert_eq!(report.invariant_failures, 0);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    fs::write(REPORT_JSON, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    fs::write(REPORT_MD, format!("# Stage 200 — algebra/number-theory composition\n\n- Cases: 240 (120 supported, 40 ambiguous, 80 unsupported)\n- Exact decisions: 240/240\n- Authorized compositions: 120/120\n- Ambiguities preserved: 40/40\n- Unsupported refusals: 80/80\n- Replay verified / tamper rejected: 240/240 / 240/240\n- Route disagreements: 0\n- Invariant failures: 0\n- False authorizations / denials: 0 / 0\n- Production registry mutations: 0\n\nThe supported routes preserve coprimality, congruence solution counts, CRT compatibility, cyclic-map kernel/image sizes, additive-order identities, and totient/unit counts. Non-coprime inverses, incompatible CRT systems, invalid maps, missing conventions, and out-of-bound rings remain closed.\n")).unwrap();
    println!("stage200 exact=240 authorized=120 ambiguous=40 unsupported=80 replay=240 tamper=240");
}
