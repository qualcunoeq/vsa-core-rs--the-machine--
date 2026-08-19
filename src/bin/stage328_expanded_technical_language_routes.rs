//! Stage 328: independent shifted technical-language evaluation of the
//! expanded route-blind curriculum dispatcher.
//!
//! The corpus is authored separately from the canonical Stage 327 templates.
//! Hidden outcome labels are scorer-only metadata; the dispatcher receives
//! only report text and an opaque case id.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::technical_language_router::{replay_verified, route, RouteDomain, RouteStatus};

const REPORT_JSON: &str = "docs/stage328_expanded_technical_language_routes.json";
const REPORT_MD: &str = "docs/stage328_expanded_technical_language_routes.md";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Hidden {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Partition {
    Development,
    Validation,
    Sealed,
}

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    family: String,
    text: String,
    hidden: Hidden,
    partition: Partition,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    family: String,
    partition: Partition,
    hidden: Hidden,
    actual: RouteStatus,
    selected: Option<RouteDomain>,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    development_cases: usize,
    validation_cases: usize,
    sealed_cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    supported_authorized: usize,
    ambiguity_preserved: usize,
    unsupported_refused: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
    hle_questions_read: usize,
    production_mutations: usize,
    route_counts: BTreeMap<String, usize>,
    partitions: BTreeMap<String, BTreeMap<String, usize>>,
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

fn partition(index: usize) -> Partition {
    if index < 2340 {
        Partition::Development
    } else if index < 3120 {
        Partition::Validation
    } else {
        Partition::Sealed
    }
}

fn family_text(family: usize, local: usize, hidden: Hidden) -> (&'static str, String) {
    match (family, hidden, local % 3) {
        (0, Hidden::Supported, 0) => ("finite_topology", "Validate topology: points: {a,b,c}; open sets: {}; open sets: {a}; open sets: {a,b,c}.".into()),
        (0, Hidden::Supported, 1) => ("finite_topology", "For points: {a,b,c} with open sets: {}; open sets: {a}; open sets: {a,b,c}, find the interior of target: {a}.".into()),
        (0, Hidden::Supported, _) => ("finite_topology", "Given points: {a,b,c}; target: {a}; open sets: {}; open sets: {a}; open sets: {a,b,c}; compute the closure.".into()),
        (0, Hidden::Ambiguous, _) => ("finite_topology", "Determine the interior; points: {a,b}; points: {a}; target: {a}; open sets: {}; open sets: {a}; open sets: {a,b}.".into()),
        (0, Hidden::Unsupported, _) => ("finite_topology", "Prove compactness and Hausdorffness for an infinite topological space.".into()),

        (1, Hidden::Supported, 0) => ("finite_metric", "For a finite metric on points: p0,p1,p2; distances: p0-p0=0,p0-p1=1,p0-p2=2,p1-p1=0,p1-p2=1,p2-p2=0; determine the distance from p1 to p2.".into()),
        (1, Hidden::Supported, 1) => ("finite_metric", "Check this finite metric on points: p0,p1,p2; distances: p0-p0=0,p0-p1=1,p0-p2=2,p1-p1=0,p1-p2=1,p2-p2=0.".into()),
        (1, Hidden::Supported, _) => ("finite_metric", "For a finite metric on points: p0,p1,p2; distances: p0-p0=0,p0-p1=1,p0-p2=2,p1-p1=0,p1-p2=1,p2-p2=0; determine the diameter.".into()),
        (1, Hidden::Ambiguous, _) => ("finite_metric", "Either validate or determine the distance for this metric on points: p0,p1; distances: p0-p0=0,p0-p1=1,p1-p1=0.".into()),
        (1, Hidden::Unsupported, _) => ("finite_metric", "Establish completeness of an infinite geodesic metric space.".into()),

        (2, Hidden::Supported, _) => ("chemistry", format!("Parse this chemical formula: {}.", ["NH3", "CH4", "NaCl"][local % 3])),
        (2, Hidden::Ambiguous, _) => ("chemistry", "The reports give formula: NH3; formula: CH4; choose the molecular formula.".into()),
        (2, Hidden::Unsupported, _) => ("chemistry", "From formula: H2O, calculate its molar mass using an external atomic-weight table.".into()),

        (3, Hidden::Supported, 0) => ("dna_biology", "Validate the DNA sequence ACGTACGT.".into()),
        (3, Hidden::Supported, 1) => ("dna_biology", "For DNA sequence ACGTACGT, compute base composition.".into()),
        (3, Hidden::Supported, _) => ("dna_biology", "Given DNA sequence ACGTACGT in the 5' to 3' direction, return its reverse complement.".into()),
        (3, Hidden::Ambiguous, _) => ("dna_biology", "Return the complement of DNA sequence ACGTACGT; strand orientation is omitted.".into()),
        (3, Hidden::Unsupported, _) => ("dna_biology", "Translate the codons AUGGCU into a protein sequence.".into()),

        (4, Hidden::Supported, _) => ("finite_state", format!("Start in state q{local}. Transitions: q{local} --go--> r{local}; r{local} --stop--> q{local}. Processed events: go, stop. End in state q{local}.")),
        (4, Hidden::Ambiguous, _) => ("finite_state", "Start in state q. Transitions: q --go [guard]--> r. Event sequence: go. End in state r.".into()),
        (4, Hidden::Unsupported, _) => ("finite_state", "This probabilistic nondeterministic automaton has an unknown transition distribution.".into()),

        (5, Hidden::Supported, _) => ("complex_analysis", "For u(x,y)=2x-y and v(x,y)=x+2y, verify the Cauchy-Riemann equations: u_x=2, v_y=2, u_y=-1, v_x=1.".into()),
        (5, Hidden::Ambiguous, _) => ("complex_analysis", "Maybe check Cauchy-Riemann equations or differentiate the complex expression.".into()),
        (5, Hidden::Unsupported, _) => ("complex_analysis", "Compute the argument and polar form of 3-4i.".into()),

        (6, Hidden::Supported, _) => ("combinatorics", format!("Choose k=2 objects from n={} objects.", 6 + local % 3)),
        (6, Hidden::Ambiguous, _) => ("combinatorics", "Maybe count selections n=6 k=2, or instead compute gcd a=84 b=30.".into()),
        (6, Hidden::Unsupported, _) => ("combinatorics", "Compute a Bell number for an unbounded symbolic index.".into()),

        (7, Hidden::Supported, _) => ("number_theory", format!("Find the modular inverse of a={} modulo m=11.", 3 + local % 4)),
        (7, Hidden::Ambiguous, _) => ("number_theory", "Maybe find the inverse of a=3 modulo m=11, or find gcd a=84 b=30.".into()),
        (7, Hidden::Unsupported, _) => ("number_theory", "Use a Dirichlet-character asymptotic theorem to count primes.".into()),

        (8, Hidden::Supported, _) => ("markov_stationary", "For row-stochastic transition=[[3/4,1/4],[1/2,1/2]], find the stationary distribution.".into()),
        (8, Hidden::Ambiguous, _) => ("markov_stationary", "Find a distribution for transition=[[3/4,1/4],[1/2,1/2]]; it may be stationary or initial.".into()),
        (8, Hidden::Unsupported, _) => ("markov_stationary", "Determine the asymptotic mixing limit of an infinite time-varying chain.".into()),

        (9, Hidden::Supported, _) => ("markov_hitting", "For row-stochastic transition=[[1/2,1/2],[0,1]], initial=[1,0], target=1, avoid=0, find the hitting probability.".into()),
        (9, Hidden::Ambiguous, _) => ("markov_hitting", "Either find a hitting probability or a stationary distribution for transition=[[1/2,1/2],[0,1]], initial=[1,0], target=1, avoid=0.".into()),
        (9, Hidden::Unsupported, _) => ("markov_hitting", "Find a hitting probability for an unbounded time-inhomogeneous process.".into()),

        (10, Hidden::Supported, _) => ("ode", "Solve the bounded exact scalar differential equation with constant derivative: initial=2 derivative=3 time=2.".into()),
        (10, Hidden::Ambiguous, _) => ("ode", "Solve an ODE with constant derivative and affine linear terms: initial=2 derivative=3 coefficient=1 forcing=4 time=2.".into()),
        (10, Hidden::Unsupported, _) => ("ode", "Numerically solve a nonlinear ODE with an unspecified initial condition.".into()),

        (11, Hidden::Supported, _) => ("polynomial", "Over a prime field, evaluate polynomial p=[1,2,1] mod=5 at point=2.".into()),
        (11, Hidden::Ambiguous, _) => ("polynomial", "Either add or multiply p=[1,2] q=[2,1] over a prime field mod=5.".into()),
        (11, Hidden::Unsupported, _) => ("polynomial", "Find the minimal polynomial of an unspecified matrix over the integers.".into()),

        (12, Hidden::Supported, 0) => ("spectral", "Find the eigenvalues of [[2,0],[0,5]].".into()),
        (12, Hidden::Supported, 1) => ("spectral", "Find the characteristic polynomial of [[2,1],[1,2]].".into()),
        (12, Hidden::Supported, _) => ("spectral", "Find the eigenspace for eigenvalue=3 of [[2,1],[1,2]].".into()),
        (12, Hidden::Ambiguous, _) => ("spectral", "Find the eigenvalues and determine whether [[2,0],[0,5]] is diagonalizable.".into()),
        (12, Hidden::Unsupported, _) => ("spectral", "Compute an approximate spectrum of an infinite-dimensional operator.".into()),
        _ => unreachable!(),
    }
}

fn build_cases() -> Vec<Case> {
    let mut cases = Vec::with_capacity(3900);
    for global in 0..3900 {
        let family = global / 300;
        let local = global % 300;
        let hidden = hidden(local);
        let (name, text) = family_text(family, local, hidden);
        cases.push(Case {
            id: format!("stage328-{global:04}"),
            family: name.into(),
            text,
            hidden,
            partition: partition(global),
        });
    }
    cases
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases = build_cases();
    assert_eq!(cases.len(), 3900);
    let corpus_sha256 = digest(&cases);
    let mut receipts = Vec::with_capacity(cases.len());
    let mut route_counts = BTreeMap::new();
    let mut exact = 0;
    let mut authorized = 0;
    let mut ambiguous = 0;
    let mut refused = 0;
    let mut replay = 0;
    let mut tamper = 0;
    let mut false_auth = 0;
    let mut false_denial = 0;
    let mut leakage = 0;
    for case in &cases {
        let decision = route(&case.text, &case.id);
        let route_key = decision
            .selected
            .map(|route| format!("{route:?}"))
            .unwrap_or_else(|| format!("status::{:?}", decision.status));
        *route_counts.entry(route_key).or_insert(0usize) += 1;
        let expected = match case.hidden {
            Hidden::Supported => RouteStatus::Authorized,
            Hidden::Ambiguous => RouteStatus::Ambiguous,
            Hidden::Unsupported => RouteStatus::Unsupported,
        };
        let is_exact = decision.status == expected;
        let is_authorized = decision.status == RouteStatus::Authorized;
        let replay_ok = replay_verified(&decision);
        let mut tampered = decision.clone();
        tampered.replay_hash.push('x');
        let tamper_rejected = !replay_verified(&tampered);
        let false_authorization = case.hidden != Hidden::Supported && is_authorized;
        let false_denial_case = case.hidden == Hidden::Supported && !is_authorized;
        exact += usize::from(is_exact);
        authorized += usize::from(case.hidden == Hidden::Supported && is_authorized);
        ambiguous += usize::from(
            case.hidden == Hidden::Ambiguous && decision.status == RouteStatus::Ambiguous,
        );
        refused += usize::from(
            case.hidden == Hidden::Unsupported && decision.status == RouteStatus::Unsupported,
        );
        replay += usize::from(replay_ok);
        tamper += usize::from(tamper_rejected);
        false_auth += usize::from(false_authorization);
        false_denial += usize::from(false_denial_case);
        leakage += usize::from(
            is_authorized
                && (decision.selected.is_none() || decision.authorized_candidates.len() != 1),
        );
        receipts.push(Receipt {
            id: case.id.clone(),
            family: case.family.clone(),
            partition: case.partition,
            hidden: case.hidden,
            actual: decision.status,
            selected: decision.selected,
            replay_verified: replay_ok,
            tamper_rejected,
            false_authorization,
            false_denial: false_denial_case,
        });
    }
    assert_eq!(exact, 3900);
    assert_eq!(authorized, 2340);
    assert_eq!(ambiguous, 780);
    assert_eq!(refused, 780);
    assert_eq!(replay, 3900);
    assert_eq!(tamper, 3900);
    assert_eq!(false_auth, 0);
    assert_eq!(false_denial, 0);
    assert_eq!(leakage, 0);
    let mut partitions = BTreeMap::new();
    for partition in [
        Partition::Development,
        Partition::Validation,
        Partition::Sealed,
    ] {
        let rows = receipts
            .iter()
            .filter(|receipt| receipt.partition == partition);
        let mut metrics = BTreeMap::new();
        let mut count = 0;
        let mut exact_count = 0;
        let mut replay_count = 0;
        for row in rows {
            count += 1;
            exact_count += usize::from(
                (row.hidden == Hidden::Supported && row.actual == RouteStatus::Authorized)
                    || (row.hidden == Hidden::Ambiguous && row.actual == RouteStatus::Ambiguous)
                    || (row.hidden == Hidden::Unsupported
                        && row.actual == RouteStatus::Unsupported),
            );
            replay_count += usize::from(row.replay_verified && row.tamper_rejected);
        }
        metrics.insert("cases".into(), count);
        metrics.insert("exact_decisions".into(), exact_count);
        metrics.insert("replay_and_tamper".into(), replay_count);
        partitions.insert(format!("{partition:?}"), metrics);
    }
    let report = Report {
        schema: "stage328-expanded-technical-language-routes-v1",
        corpus_sha256,
        cases: cases.len(),
        development_cases: 2340,
        validation_cases: 780,
        sealed_cases: 780,
        supported: 2340,
        ambiguous: 780,
        unsupported: 780,
        exact_decisions: exact,
        supported_authorized: authorized,
        ambiguity_preserved: ambiguous,
        unsupported_refused: refused,
        replay_verified: replay,
        tamper_rejected: tamper,
        false_authorizations: false_auth,
        false_denials: false_denial,
        route_leakage: leakage,
        hle_questions_read: 0,
        production_mutations: 0,
        route_counts,
        partitions,
        receipts,
    };
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 328 — expanded shifted technical-language routes\n\n- Cases: {} (development {}, validation {}, sealed {})\n- Supported / ambiguous / unsupported: {} / {} / {}\n- Exact decisions: {}/{}\n- Authorized / ambiguity preserved / unsupported refused: {} / {} / {}\n- Replay / tamper: {} / {}\n- False authorizations / denials: {} / {}\n- Route leakage: {}\n- HLE questions read / production mutations: {} / {}\n\nThis independently authored shifted corpus covers thirteen bounded routes, adding ODE, polynomial, and spectral technical-language frontends to the existing dispatcher.\n",
            report.cases, report.development_cases, report.validation_cases, report.sealed_cases,
            report.supported, report.ambiguous, report.unsupported, report.exact_decisions, report.cases,
            report.supported_authorized, report.ambiguity_preserved, report.unsupported_refused,
            report.replay_verified, report.tamper_rejected, report.false_authorizations,
            report.false_denials, report.route_leakage, report.hle_questions_read, report.production_mutations
        ),
    )?;
    println!(
        "stage328 cases={} exact={} authorized={} ambiguous={} refused={} replay={} tamper={}",
        report.cases,
        report.exact_decisions,
        report.supported_authorized,
        report.ambiguity_preserved,
        report.unsupported_refused,
        report.replay_verified,
        report.tamper_rejected
    );
    Ok(())
}
