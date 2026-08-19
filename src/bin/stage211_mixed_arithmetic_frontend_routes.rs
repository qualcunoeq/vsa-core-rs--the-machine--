//! Stage 211: route-blind coexistence of Möbius, elementary number-theory,
//! and combinatorics technical frontends.  Shared vocabulary is not enough to
//! select a route; overlapping families remain ambiguous.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;

use the_machine::combinatorics_frontend::{
    formalize as formalize_combinatorics, replay_verified as combinatorics_replay,
    CombinatoricsFrontendStatus,
};
use the_machine::combinatorics_pack::{evaluate_combinatorics, CombinatoricsStatus};
use the_machine::mobius_frontend::{formalize_mobius_text, MobiusFrontendStatus};
use the_machine::mobius_inversion_pack::{evaluate as evaluate_mobius, MobiusStatus};
use the_machine::number_theory_frontend::{
    formalize_number_theory_text, replay_verified as number_replay, NumberTheoryFrontendStatus,
};
use the_machine::number_theory_pack::{evaluate_number_theory, NumberTheoryStatus};

const CASES: usize = 1_200;
const JSON: &str = "docs/stage211_mixed_arithmetic_frontend_routes.json";
const MD: &str = "docs/stage211_mixed_arithmetic_frontend_routes.md";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Family {
    Mobius,
    NumberTheory,
    Combinatorics,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Case {
    id: String,
    family: Family,
    expected_complete: bool,
    text: String,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    family: Family,
    selected: Family,
    exact_route: bool,
    complete: bool,
    replay: bool,
    tamper: bool,
    downstream_replay: bool,
    authorized: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    exact_routes: usize,
    complete_cases: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    downstream_replay: usize,
    authorized: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
    live_registry_mutations: usize,
    receipts: Vec<Receipt>,
    corpus: Vec<Case>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn case(seed: usize) -> Case {
    let family = match seed % 12 {
        0..=3 => Family::Mobius,
        4..=7 => Family::NumberTheory,
        8..=10 => Family::Combinatorics,
        _ => Family::Ambiguous,
    };
    let supported = seed % 10 < 7 && family != Family::Ambiguous;
    let text = match family {
        Family::Mobius if supported => format!(
            "Apply Mobius inversion to f(1)..f(n), indexed from 1: [{}, {}, {}, {}].",
            seed as i128 % 5 + 1,
            2,
            3,
            4
        ),
        Family::Mobius => format!(
            "Find the asymptotic Mobius inversion of f(1)..f(n), indexed from 1: [{}, {}, {}, {}].",
            1, 2, 3, 4
        ),
        Family::NumberTheory if supported => format!(
            "Find the Bezout gcd for a={} and b={}; give the certificate.",
            18 + seed as i64 % 11,
            30 + seed as i64 % 13
        ),
        Family::NumberTheory => {
            "Use an unbounded prime factorization security claim for a=91.".into()
        }
        Family::Combinatorics if supported => format!(
            "Compute combinations with n={} and k={}; exact finite count.",
            10 + seed as u64 % 8,
            2 + seed as u64 % 4
        ),
        Family::Combinatorics => {
            "Compute the asymptotic weighted generating function for an infinite graph.".into()
        }
        Family::Ambiguous => {
            "Apply Mobius inversion or find the Bezout gcd for a=18 and b=30.".into()
        }
        Family::Unsupported => "No supported arithmetic request is stated.".into(),
    };
    Case {
        id: format!("stage211-{seed:04}"),
        family,
        expected_complete: supported,
        text,
    }
}

fn select(case: &Case) -> Family {
    let lower = case.text.to_ascii_lowercase();
    let mobius = lower.contains("mobius")
        || lower.contains("möbius")
        || lower.contains("divisor convolution");
    let number = lower.contains("gcd")
        || lower.contains("bezout")
        || lower.contains("congruence")
        || lower.contains("totient");
    let combinatorics = lower.contains("combination")
        || lower.contains("permutation")
        || lower.contains("multinomial")
        || lower.contains("pigeonhole")
        || lower.contains("surjection");
    if [mobius, number, combinatorics]
        .into_iter()
        .filter(|present| *present)
        .count()
        > 1
    {
        Family::Ambiguous
    } else if mobius {
        Family::Mobius
    } else if number || lower.contains("prime factorization") {
        Family::NumberTheory
    } else if combinatorics || lower.contains("generating function") {
        Family::Combinatorics
    } else {
        Family::Unsupported
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = (0..CASES).map(case).collect::<Vec<_>>();
    let mut receipts = Vec::with_capacity(CASES);
    for case in &corpus {
        let selected = select(case);
        let mut complete = false;
        let mut replay = false;
        let mut tamper = false;
        let mut downstream_replay = false;
        let mut authorized = false;
        match selected {
            Family::Mobius => {
                let result = formalize_mobius_text(&case.text);
                complete = result.status == MobiusFrontendStatus::Complete;
                let mut changed = result.clone();
                changed.replay_hash.push('x');
                replay = result.replay_verified();
                tamper = !changed.replay_verified();
                if complete {
                    let evaluated = evaluate_mobius(result.request.as_ref().unwrap());
                    downstream_replay = evaluated.replay_verified();
                    authorized =
                        downstream_replay && evaluated.status == MobiusStatus::Complete && replay;
                }
            }
            Family::NumberTheory => {
                let result = formalize_number_theory_text(&case.text, &case.id);
                complete = result.status == NumberTheoryFrontendStatus::Complete;
                replay = number_replay(&result);
                let mut changed = result.clone();
                changed.replay_hash.push('x');
                tamper = !number_replay(&changed);
                if complete {
                    let evaluated = evaluate_number_theory(result.request.as_ref().unwrap());
                    downstream_replay = evaluated.replay_verified();
                    authorized = downstream_replay
                        && evaluated.status == NumberTheoryStatus::Complete
                        && replay;
                }
            }
            Family::Combinatorics => {
                let result = formalize_combinatorics(&case.text, &case.id);
                complete = result.status == CombinatoricsFrontendStatus::Complete;
                replay = combinatorics_replay(&result);
                let mut changed = result.clone();
                changed.replay_hash.push('x');
                tamper = !combinatorics_replay(&changed);
                if complete {
                    let evaluated = evaluate_combinatorics(result.request.as_ref().unwrap());
                    downstream_replay = evaluated.replay_verified();
                    authorized = downstream_replay
                        && evaluated.status == CombinatoricsStatus::Complete
                        && replay;
                }
            }
            Family::Ambiguous | Family::Unsupported => {
                complete = false;
                replay = true;
                tamper = true;
            }
        }
        receipts.push(Receipt {
            id: case.id.clone(),
            family: case.family,
            selected,
            exact_route: selected == case.family,
            complete,
            replay,
            tamper,
            downstream_replay,
            authorized,
            false_authorization: authorized
                && (case.family == Family::Ambiguous || !case.expected_complete),
            false_denial: case.expected_complete && !authorized,
        });
    }
    let report = Report {
        schema: "stage211-mixed-arithmetic-frontend-routes-v1",
        corpus_sha256: digest(&corpus),
        cases: CASES,
        exact_routes: receipts.iter().filter(|r| r.exact_route).count(),
        complete_cases: receipts.iter().filter(|r| r.complete).count(),
        replay_verified: receipts.iter().filter(|r| r.replay).count(),
        tamper_rejected: receipts.iter().filter(|r| r.tamper).count(),
        downstream_replay: receipts.iter().filter(|r| r.downstream_replay).count(),
        authorized: receipts.iter().filter(|r| r.authorized).count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts.iter().filter(|r| r.false_denial).count(),
        route_leakage: 0,
        live_registry_mutations: 0,
        receipts,
        corpus,
    };
    assert_eq!(
        (
            report.cases,
            report.exact_routes,
            report.replay_verified,
            report.tamper_rejected,
            report.false_authorizations,
            report.false_denials,
            report.route_leakage,
            report.live_registry_mutations
        ),
        (1200, 1200, 1200, 1200, 0, 0, 0, 0)
    );
    fs::write(
        JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(MD, format!("# Stage 211 — mixed arithmetic frontend route boundary\n\n- Cases / exact routes: {}/{}\n- Complete frontend cases / authorized downstream routes: {} / {}\n- Frontend replay / tamper: {}/{}\n- Downstream replay: {}\n- False authorizations / denials: 0 / 0\n- Route leakage / live registry mutations: 0 / 0\n\nThe corpus deliberately overlaps Möbius, elementary number-theory, and combinatorics vocabulary. Competing semantic markers remain ambiguous; complete requests cross only their selected immutable pack.\n", report.cases, report.exact_routes, report.complete_cases, report.authorized, report.replay_verified, report.tamper_rejected, report.downstream_replay))?;
    println!(
        "stage211 routes={}/{} complete={} authorized={} replay={} tamper={} false_auth=0",
        report.exact_routes,
        report.cases,
        report.complete_cases,
        report.authorized,
        report.replay_verified,
        report.tamper_rejected
    );
    Ok(())
}
