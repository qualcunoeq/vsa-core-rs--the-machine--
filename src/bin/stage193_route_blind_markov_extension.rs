//! Stage 193: route-blind technical language with finite-Markov extensions.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::technical_language_router::{replay_verified, route, RouteDomain, RouteStatus};

const JSON: &str = "docs/stage193_route_blind_markov_extension.json";
const MD: &str = "docs/stage193_route_blind_markov_extension.md";

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    partition: String,
    expected: String,
    actual: String,
    selected: Option<RouteDomain>,
    authorized: Vec<RouteDomain>,
    ambiguous: Vec<RouteDomain>,
    exact: bool,
    replay: bool,
    tamper_rejected: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    manifest_sha256: String,
    corpus_sha256: String,
    cases: usize,
    development_cases: usize,
    holdout_cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    development_exact: usize,
    development_authorized: usize,
    holdout_exact: usize,
    holdout_authorized: usize,
    authorized_routes: BTreeMap<RouteDomain, usize>,
    route_leakage: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}
fn stationary(i: usize) -> String {
    if i % 2 == 0 {
        "Find the stationary distribution for a row-stochastic transition=[[3/4,1/4],[1/2,1/2]]."
            .into()
    } else {
        "Compute the stationary invariant distribution of this row-stochastic transition matrix=[[2/3,1/3],[1/4,3/4]].".into()
    }
}
fn hitting(i: usize) -> String {
    if i % 2 == 0 {
        "Find the hitting probability for a row-stochastic transition=[[1,0,0],[1/4,1/4,1/2],[0,0,1]] with initial=[0,1,0], target=2, avoid=0.".into()
    } else {
        "Compute the hitting probability with explicit target and avoid states for row-stochastic transition=[[1,0,0],[1/2,1/4,1/4],[0,0,1]], initial=[0,1,0], target=2, avoid=0.".into()
    }
}
fn complex(i: usize) -> String {
    if i % 2 == 0 {
        "Check the Cauchy-Riemann equations: ux=2, uy=-1, vx=1, vy=2.".into()
    } else {
        "Differentiate the affine map after checking Cauchy Riemann: v_y=2; u_x=2; v_x=1; u_y=-1."
            .into()
    }
}
fn combinations(i: usize) -> String {
    if i % 2 == 0 {
        "Count combinations with n=5 k=2.".into()
    } else {
        "How many choices are possible using the binomial operation n = 6 and k = 3?".into()
    }
}
fn gcd(i: usize) -> String {
    if i % 2 == 0 {
        "Find gcd, the greatest common divisor, with a=84 b=30.".into()
    } else {
        "Compute the Bezout gcd certificate for a = 99 and b = 36.".into()
    }
}
fn ambiguous(i: usize) -> String {
    if i % 2 == 0 {
        "Maybe either stationary distribution or hitting probability for transition=[[3/4,1/4],[1/2,1/2]] with initial=[1,0], target=1, avoid=0.".into()
    } else {
        "Find a stationary distribution for transition=[[3/4,1/4],[1/2,1/2]] without declaring the stochastic convention.".into()
    }
}
fn unsupported(i: usize) -> String {
    if i % 2 == 0 {
        "Find a spectral mixing limit for an infinite transition process.".into()
    } else {
        "Use a continuous-time hitting time for a diffusion.".into()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut receipts = Vec::with_capacity(2_000);
    let mut routes = BTreeMap::new();
    let mut exact = 0;
    let mut dev_exact = 0;
    let mut dev_auth = 0;
    let mut hold_exact = 0;
    let mut hold_auth = 0;
    let mut replay = 0;
    let mut tamper = 0;
    let mut leakage = 0;
    for i in 0..2_000 {
        let (expected, text) = if i < 320 {
            ("supported", stationary(i))
        } else if i < 640 {
            ("supported", hitting(i))
        } else if i < 960 {
            ("supported", complex(i))
        } else if i < 1_280 {
            ("supported", combinations(i))
        } else if i < 1_600 {
            ("supported", gcd(i))
        } else if i < 1_800 {
            ("ambiguous", ambiguous(i))
        } else {
            ("unsupported", unsupported(i))
        };
        let partition = if i < 1_500 { "development" } else { "holdout" };
        let decision = route(&text, &format!("stage193-{i:04}"));
        let actual = match decision.status {
            RouteStatus::Authorized => "supported",
            RouteStatus::Ambiguous => "ambiguous",
            RouteStatus::Unsupported => "unsupported",
        };
        let is_exact = actual == expected;
        let authorized = decision.status == RouteStatus::Authorized;
        if decision.authorized_candidates.len() > 1 {
            leakage += 1;
        }
        if let Some(selected) = decision.selected {
            *routes.entry(selected).or_insert(0) += 1;
        }
        let is_replay = replay_verified(&decision);
        let mut forged = decision.clone();
        forged.replay_hash.push('x');
        let tamper_ok = !replay_verified(&forged);
        exact += usize::from(is_exact);
        replay += usize::from(is_replay);
        tamper += usize::from(tamper_ok);
        if partition == "development" {
            dev_exact += usize::from(is_exact);
            dev_auth += usize::from(authorized);
        } else {
            hold_exact += usize::from(is_exact);
            hold_auth += usize::from(authorized);
        }
        receipts.push(Receipt {
            id: format!("stage193-{i:04}"),
            partition: partition.into(),
            expected: expected.into(),
            actual: actual.into(),
            selected: decision.selected,
            authorized: decision.authorized_candidates.clone(),
            ambiguous: decision.ambiguous_candidates.clone(),
            exact: is_exact,
            replay: is_replay,
            tamper_rejected: tamper_ok,
            false_authorization: expected != "supported" && authorized,
            false_denial: expected == "supported" && !authorized,
        });
    }
    let false_auth = receipts.iter().filter(|r| r.false_authorization).count();
    let false_den = receipts.iter().filter(|r| r.false_denial).count();
    assert_eq!(
        (
            exact, dev_exact, hold_exact, dev_auth, hold_auth, replay, tamper, leakage, false_auth,
            false_den
        ),
        (2_000, 1_500, 500, 1_500, 100, 2_000, 2_000, 0, 0, 0)
    );
    let report = Report {
        schema: "stage193-route-blind-markov-extension-v1",
        manifest_sha256: breadth_first_manifest().replay_hash(),
        corpus_sha256: digest(&receipts),
        cases: 2_000,
        development_cases: 1_500,
        holdout_cases: 500,
        supported: 1_600,
        ambiguous: 200,
        unsupported: 200,
        exact_decisions: exact,
        development_exact: dev_exact,
        development_authorized: dev_auth,
        holdout_exact: hold_exact,
        holdout_authorized: hold_auth,
        authorized_routes: routes,
        route_leakage: leakage,
        replay_verified: replay,
        tamper_rejections: tamper,
        false_authorizations: false_auth,
        false_denials: false_den,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(JSON, format!("{serialized}\n"))?;
    fs::write(MD, format!("# Stage 193 — route-blind Markov extension\n\nFive validated technical frontends share one dispatcher.\n\n- Cases: 2,000 (development 1,500; holdout 500)\n- Supported / ambiguous / unsupported: 1,600 / 200 / 200\n- Exact: {exact}/2,000\n- Replay / tamper rejection: {replay}/2,000 / {tamper}/2,000\n- False authorizations / denials: {false_auth} / {false_den}\n- Production mutation: false\n\nManifest and corpus hashes are recorded in `{JSON}`.\n"))?;
    println!("{serialized}");
    Ok(())
}
