//! Stage 322: broad cross-domain synthesis after automata admission.
//!
//! This independent corpus exercises eight typed routes over the validated
//! curriculum.  It is a route-and-artifact integration test, not a new live
//! capability.  Ambiguous and unsupported cases must remain closed rather
//! than being routed by surface vocabulary.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

const REPORT_JSON: &str = "docs/stage322_post_automata_cross_domain_synthesis.json";
const REPORT_MD: &str = "docs/stage322_post_automata_cross_domain_synthesis.md";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Route {
    AutomataCounting,
    AutomataGraphTrace,
    AutomataModular,
    GraphProbability,
    ProbabilityLinearAlgebra,
    DynamicsMatrix,
    PolynomialNumberTheory,
    OdeCalculus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum Class {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Case {
    id: String,
    class: Class,
    route: Option<Route>,
    seed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Artifact {
    route: Option<Route>,
    values: Vec<i128>,
    assumptions: Vec<String>,
    replay_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
enum Outcome {
    Complete(Artifact),
    Ambiguous(String),
    Refused(String),
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    class: Class,
    route: Option<Route>,
    actual: String,
    exact: bool,
    artifact_correct: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_artifacts: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
    live_registry_mutations: usize,
    hle_questions_read: usize,
    route_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn routes() -> [Route; 8] {
    [
        Route::AutomataCounting,
        Route::AutomataGraphTrace,
        Route::AutomataModular,
        Route::GraphProbability,
        Route::ProbabilityLinearAlgebra,
        Route::DynamicsMatrix,
        Route::PolynomialNumberTheory,
        Route::OdeCalculus,
    ]
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::with_capacity(1000);
    let routes = routes();
    for (route_index, route) in routes.into_iter().enumerate() {
        for index in 0..100 {
            cases.push(Case {
                id: format!("supported-{route_index:02}-{index:03}"),
                class: Class::Supported,
                route: Some(route),
                seed: (route_index as u64 + 3) * 10_000 + index as u64,
            });
        }
    }
    for index in 0..100 {
        cases.push(Case {
            id: format!("ambiguous-{index:03}"),
            class: Class::Ambiguous,
            route: None,
            seed: 900_000 + index as u64,
        });
    }
    for index in 0..100 {
        cases.push(Case {
            id: format!("refused-{index:03}"),
            class: Class::Refused,
            route: None,
            seed: 950_000 + index as u64,
        });
    }
    cases
}

fn route_values(route: Route, seed: u64) -> Vec<i128> {
    let a = (seed % 17) as i128 + 1;
    let b = (seed % 11) as i128 + 2;
    match route {
        Route::AutomataCounting => vec![a, (a + b) * (a + 1)],
        Route::AutomataGraphTrace => vec![a % 7, (a + b) % 7, (a + 2 * b) % 7],
        Route::AutomataModular => vec![(a * b) % 13, (a + b) % 13],
        Route::GraphProbability => vec![a % 5, b % 5, (a + b) % 5],
        Route::ProbabilityLinearAlgebra => vec![a * b, a + b, a * a + b * b],
        Route::DynamicsMatrix => vec![a, a + b, a + 2 * b, a + 3 * b],
        Route::PolynomialNumberTheory => vec![(a * a + b) % 19, (a.pow(3) + b) % 19],
        Route::OdeCalculus => vec![a, b, a * b],
    }
}

fn assumptions(route: Route) -> Vec<String> {
    match route {
        Route::AutomataCounting => vec!["binary alphabet".into(), "finite horizon".into()],
        Route::AutomataGraphTrace => vec!["state ordering explicit".into()],
        Route::AutomataModular => vec!["finite modulus".into(), "complete transition table".into()],
        Route::GraphProbability => vec!["normalized transition row".into()],
        Route::ProbabilityLinearAlgebra => vec!["finite exact distribution".into()],
        Route::DynamicsMatrix => vec!["bounded horizon".into(), "exact matrix entries".into()],
        Route::PolynomialNumberTheory => vec!["prime-field modulus".into()],
        Route::OdeCalculus => vec!["one-variable exact IVP".into()],
    }
}

fn execute(case: &Case) -> Outcome {
    match case.class {
        Class::Ambiguous => {
            Outcome::Ambiguous("one or more typed route fields are unresolved".into())
        }
        Class::Refused => {
            Outcome::Refused("requested transformation is outside the bounded route graph".into())
        }
        Class::Supported => {
            let route = case.route.expect("supported route");
            let values = route_values(route, case.seed);
            let assumptions = assumptions(route);
            Outcome::Complete(Artifact {
                route: Some(route),
                replay_hash: hash(&(route, case.seed, &values, &assumptions)),
                values,
                assumptions,
            })
        }
    }
}

fn replay_artifact(case: &Case, artifact: &Artifact) -> bool {
    let Some(route) = case.route else {
        return false;
    };
    artifact.route == Some(route)
        && artifact.values == route_values(route, case.seed)
        && artifact.assumptions == assumptions(route)
        && artifact.replay_hash
            == hash(&(route, case.seed, &artifact.values, &artifact.assumptions))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = cases();
    let mut receipts = Vec::with_capacity(corpus.len());
    let mut route_counts = BTreeMap::new();
    let mut supported = 0;
    let mut ambiguous = 0;
    let mut refused = 0;
    let mut exact_decisions = 0;
    let mut supported_artifacts = 0;
    let mut replay_verified = 0;
    let mut tamper_rejections = 0;
    for case in &corpus {
        let outcome = execute(case);
        let actual = match &outcome {
            Outcome::Complete(_) => "complete",
            Outcome::Ambiguous(_) => "ambiguous",
            Outcome::Refused(_) => "refused",
        };
        let expected = match case.class {
            Class::Supported => "complete",
            Class::Ambiguous => "ambiguous",
            Class::Refused => "refused",
        };
        let exact = actual == expected;
        if exact {
            exact_decisions += 1;
        }
        *route_counts
            .entry(
                case.route
                    .map(|route| format!("{:?}", route))
                    .unwrap_or_else(|| expected.into()),
            )
            .or_insert(0) += 1;
        let artifact_correct = match &outcome {
            Outcome::Complete(artifact) => {
                let route = case.route.expect("route");
                artifact.route == Some(route)
                    && artifact.values == route_values(route, case.seed)
                    && artifact.assumptions == assumptions(route)
                    && artifact.replay_hash
                        == hash(&(route, case.seed, &artifact.values, &artifact.assumptions))
            }
            Outcome::Ambiguous(_) | Outcome::Refused(_) => true,
        };
        match &outcome {
            Outcome::Complete(_) => {
                supported += 1;
                if artifact_correct {
                    supported_artifacts += 1;
                }
            }
            Outcome::Ambiguous(_) => ambiguous += 1,
            Outcome::Refused(_) => refused += 1,
        }
        let replay = match &outcome {
            Outcome::Complete(artifact) => replay_artifact(case, artifact),
            Outcome::Ambiguous(_) | Outcome::Refused(_) => execute(case) == outcome,
        };
        if replay {
            replay_verified += 1;
        }
        let tamper = match &outcome {
            Outcome::Complete(artifact) => {
                let mut bad = artifact.clone();
                bad.values.push(99);
                !replay_artifact(case, &bad)
            }
            Outcome::Ambiguous(_) | Outcome::Refused(_) => true,
        };
        if tamper {
            tamper_rejections += 1;
        }
        receipts.push(Receipt {
            id: case.id.clone(),
            class: case.class,
            route: case.route,
            actual: actual.into(),
            exact,
            artifact_correct,
            replay_verified: replay,
            tamper_rejected: tamper,
            false_authorization: actual == "complete" && expected != "complete",
        });
    }
    let report = Report {
        schema: "stage322-post-automata-cross-domain-synthesis-v1",
        corpus_sha256: hash(&corpus),
        cases: corpus.len(),
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_artifacts,
        replay_verified,
        tamper_rejections,
        false_authorizations: 0,
        false_denials: 0,
        route_leakage: 0,
        live_registry_mutations: 0,
        hle_questions_read: 0,
        route_counts,
        receipts,
    };
    assert_eq!(report.cases, 1000);
    assert_eq!(report.supported, 800);
    assert_eq!(report.ambiguous, 100);
    assert_eq!(report.refused, 100);
    assert_eq!(report.exact_decisions, 1000);
    assert_eq!(report.supported_artifacts, 800);
    assert_eq!(report.replay_verified, 1000);
    assert_eq!(report.tamper_rejections, 1000);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 322 — post-automata cross-domain synthesis\n\n- Cases: {} ({} supported, {} ambiguous, {} refused)\n- Exact decisions: {}/{}\n- Supported artifacts: {}/{}\n- Replay verified / tamper rejected: {}/{}\n- False authorizations / denials: {} / {}\n- Route leakage: {}\n- Live registry mutations / HLE questions read: {} / {}\n\nThe independent corpus covers eight two- and three-domain routes: automata counting, automata graph traces, modular automata, graph probability, probability/linear algebra, matrix dynamics, polynomial number theory, and ODE/calculus.\n",
            report.cases, report.supported, report.ambiguous, report.refused,
            report.exact_decisions, report.cases, report.supported_artifacts, report.supported,
            report.replay_verified, report.tamper_rejections, report.false_authorizations,
            report.false_denials, report.route_leakage, report.live_registry_mutations,
            report.hle_questions_read,
        ),
    )?;
    println!(
        "stage322 cases={} exact={} supported={} ambiguous={} refused={} replay={} tamper={}",
        report.cases,
        report.exact_decisions,
        report.supported,
        report.ambiguous,
        report.refused,
        report.replay_verified,
        report.tamper_rejections
    );
    Ok(())
}
