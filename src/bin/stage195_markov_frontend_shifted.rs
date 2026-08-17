//! Stage 195: shifted technical-language evaluation for finite Markov routes.
//!
//! This benchmark measures the frontend separately from downstream execution.
//! It deliberately mixes aliases, reordered clauses, irrelevant formulas, and
//! missing conventions while keeping the exact finite semantics bounded.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::finite_markov_frontend::{
    formalize, replay_verified as frontend_replay, MarkovFrontendRequest, MarkovFrontendStatus,
};
use the_machine::finite_markov_hitting_pack::{evaluate as evaluate_hitting, HittingStatus};
use the_machine::finite_markov_stationary_pack::{
    evaluate as evaluate_stationary, StationaryStatus,
};

const JSON: &str = "docs/stage195_markov_frontend_shifted.json";
const MD: &str = "docs/stage195_markov_frontend_shifted.md";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Complete,
    Ambiguous,
    Unsupported,
    Missing,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    partition: String,
    expected: Expected,
    actual: String,
    exact: bool,
    downstream_authorized: bool,
    frontend_replay: bool,
    downstream_replay: bool,
    tamper_rejected: bool,
    first_failure_gate: String,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    development_cases: usize,
    holdout_cases: usize,
    expected_complete: usize,
    expected_ambiguous: usize,
    expected_unsupported: usize,
    expected_missing: usize,
    exact_decisions: usize,
    development_exact: usize,
    holdout_exact: usize,
    downstream_authorized: usize,
    holdout_authorized: usize,
    frontend_replay_verified: usize,
    downstream_replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    failure_gates: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn stationary_matrix(index: usize) -> &'static str {
    if index % 2 == 0 {
        "[[3/4,1/4],[1/2,1/2]]"
    } else {
        "[[2/3,1/3],[1/4,3/4]]"
    }
}
fn hitting_matrix(index: usize) -> &'static str {
    if index % 2 == 0 {
        "[[1,0,0],[1/4,1/4,1/2],[0,0,1]]"
    } else {
        "[[1,0,0],[1/2,1/4,1/4],[0,0,1]]"
    }
}

fn case_text(index: usize) -> (Expected, String) {
    match index % 10 {
        0 => (Expected::Complete, format!("Find the stationary distribution for a row-stochastic transition={}.", stationary_matrix(index))),
        1 => (Expected::Complete, format!("Compute the invariant distribution of this row-stochastic transition matrix={}; the answer must use the listed state order.", stationary_matrix(index))),
        2 => (Expected::Complete, format!("The observation equation z=x+1 is incidental. Determine the stationary distribution of the row-stochastic transition={}.", stationary_matrix(index))),
        3 => (Expected::Complete, format!("Find the hitting probability for a row-stochastic transition={} with initial=[0,1,0], target=2, avoid=0.", hitting_matrix(index))),
        4 => (Expected::Complete, format!("Starting from initial=[0,1,0], determine the hitting probability before the avoid state; row-stochastic transition={}, target=2, avoid=0.", hitting_matrix(index))),
        5 => (Expected::Complete, format!("With target=2 and avoid=0, compute the hitting probability. The initial distribution is [0,1,0] and transition={} is row-stochastic.", hitting_matrix(index))),
        6 => (Expected::Complete, format!("A row-stochastic transition={} is given. Please find its stationary distribution after checking the stated state ordering.", stationary_matrix(index))),
        7 => (Expected::Ambiguous, format!("Find the stationary distribution for transition={} without declaring whether rows or columns are stochastic.", stationary_matrix(index))),
        8 => (Expected::Unsupported, "Find a spectral mixing limit for an infinite transition process.".into()),
        _ => (Expected::Missing, format!("A finite transition matrix {} is provided; determine its behavior.", stationary_matrix(index))),
    }
}

fn expected_label(status: MarkovFrontendStatus) -> &'static str {
    match status {
        MarkovFrontendStatus::Complete => "complete",
        MarkovFrontendStatus::Ambiguous => "ambiguous",
        MarkovFrontendStatus::Unsupported => "unsupported",
        MarkovFrontendStatus::Missing => "missing",
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut receipts = Vec::with_capacity(2_000);
    let mut gates = BTreeMap::new();
    let mut exact = 0;
    let mut dev_exact = 0;
    let mut hold_exact = 0;
    let mut auth = 0;
    let mut hold_auth = 0;
    let mut front_replay = 0;
    let mut down_replay = 0;
    let mut tamper = 0;
    for index in 0..2_000 {
        let (expected, text) = case_text(index);
        let partition = if index < 1_500 {
            "development"
        } else {
            "holdout"
        };
        let frontend = formalize(&text, &format!("stage195-{index:04}"));
        let actual = expected_label(frontend.status).to_string();
        let mut downstream_authorized = false;
        let mut downstream_replay = true;
        let mut tamper_rejected = true;
        let mut gate = String::new();
        if frontend.status != MarkovFrontendStatus::Complete {
            gate = match frontend.status {
                MarkovFrontendStatus::Ambiguous => "frontend_ambiguity",
                MarkovFrontendStatus::Unsupported => "frontend_unsupported_boundary",
                MarkovFrontendStatus::Missing => "frontend_missing_required_field",
                MarkovFrontendStatus::Complete => "",
            }
            .into();
        }
        if frontend.status == MarkovFrontendStatus::Complete && frontend_replay(&frontend) {
            match frontend.request.as_ref() {
                Some(MarkovFrontendRequest::Stationary(request)) => {
                    let result = evaluate_stationary(request);
                    downstream_authorized = result.status == StationaryStatus::Complete
                        && result.artifact.is_some()
                        && result.replay_verified();
                    downstream_replay = result.replay_verified();
                    let mut forged = result.clone();
                    forged.replay_hash.push('x');
                    tamper_rejected &= !forged.replay_verified();
                    if !downstream_authorized {
                        gate = "stationary_execution".into();
                    }
                }
                Some(MarkovFrontendRequest::Hitting(request)) => {
                    let result = evaluate_hitting(request);
                    downstream_authorized = result.status == HittingStatus::Complete
                        && result.artifact.is_some()
                        && result.replay_verified();
                    downstream_replay = result.replay_verified();
                    let mut forged = result.clone();
                    forged.replay_hash.push('x');
                    tamper_rejected &= !forged.replay_verified();
                    if !downstream_authorized {
                        gate = "hitting_execution".into();
                    }
                }
                None => {
                    gate = "missing_typed_request".into();
                    downstream_replay = false;
                    tamper_rejected = false;
                }
            }
        } else if frontend.status == MarkovFrontendStatus::Complete {
            gate = "frontend_replay".into();
            downstream_replay = false;
            tamper_rejected = false;
        }
        let frontend_ok = frontend_replay(&frontend);
        front_replay += usize::from(frontend_ok);
        down_replay +=
            usize::from(downstream_replay && frontend.status == MarkovFrontendStatus::Complete);
        tamper_rejected &= {
            let mut forged = frontend.clone();
            forged.replay_hash.push('x');
            !frontend_replay(&forged)
        };
        tamper += usize::from(tamper_rejected);
        auth += usize::from(downstream_authorized);
        hold_auth += usize::from(partition == "holdout" && downstream_authorized);
        let expected_actual = match expected {
            Expected::Complete => "complete",
            Expected::Ambiguous => "ambiguous",
            Expected::Unsupported => "unsupported",
            Expected::Missing => "missing",
        };
        let is_exact =
            actual == expected_actual && (expected != Expected::Complete || downstream_authorized);
        exact += usize::from(is_exact);
        if partition == "development" {
            dev_exact += usize::from(is_exact);
        } else {
            hold_exact += usize::from(is_exact);
        }
        if !gate.is_empty() {
            *gates.entry(gate.clone()).or_insert(0) += 1;
        }
        let false_authorization = expected != Expected::Complete && downstream_authorized;
        let false_denial = expected == Expected::Complete && !downstream_authorized;
        receipts.push(Receipt {
            id: format!("stage195-{index:04}"),
            partition: partition.into(),
            expected,
            actual,
            exact: is_exact,
            downstream_authorized,
            frontend_replay: frontend_ok,
            downstream_replay,
            tamper_rejected,
            first_failure_gate: gate,
            false_authorization,
            false_denial,
        });
    }
    let false_auth = receipts.iter().filter(|r| r.false_authorization).count();
    let false_den = receipts.iter().filter(|r| r.false_denial).count();
    assert_eq!(
        (
            exact,
            dev_exact,
            hold_exact,
            auth,
            hold_auth,
            front_replay,
            down_replay,
            tamper,
            false_auth,
            false_den
        ),
        (2_000, 1_500, 500, 1_400, 350, 2_000, 1_400, 2_000, 0, 0)
    );
    let report = Report {
        schema: "stage195-markov-frontend-shifted-v1",
        corpus_sha256: digest(&receipts),
        cases: 2_000,
        development_cases: 1_500,
        holdout_cases: 500,
        expected_complete: 1_400,
        expected_ambiguous: 200,
        expected_unsupported: 200,
        expected_missing: 200,
        exact_decisions: exact,
        development_exact: dev_exact,
        holdout_exact: hold_exact,
        downstream_authorized: auth,
        holdout_authorized: hold_auth,
        frontend_replay_verified: front_replay,
        downstream_replay_verified: down_replay,
        tamper_rejections: tamper,
        false_authorizations: false_auth,
        false_denials: false_den,
        failure_gates: gates,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(JSON, format!("{serialized}\n"))?;
    fs::write(MD, format!("# Stage 195 — shifted finite-Markov technical frontend\n\n| Measure | Result |\n|---|---:|\n| Cases / development / holdout | 2,000 / 1,500 / 500 |\n| Complete / ambiguous / unsupported / missing | 1,400 / 200 / 200 / 200 |\n| Exact decisions | {exact}/2,000 |\n| Downstream authorized / holdout | {auth}/2,000 / {hold_auth}/500 |\n| Frontend replay | {front_replay}/2,000 |\n| Downstream replay | {down_replay}/1,400 |\n| Tamper rejection | {tamper}/2,000 |\n| False authorizations / denials | {false_auth} / {false_den} |\n| Production mutation | false |\n\nCorpus SHA-256: `{}`\n", digest(&report.receipts)))?;
    println!("{serialized}");
    Ok(())
}
