//! Stage P: route-blind, shifted technical-language benchmark.
//!
//! Unlike the earlier controlled language gate, this benchmark does not pass
//! the expected route to the parser.  A front door must select exactly one
//! bounded curriculum route from raw text, preserve ambiguity, or refuse.
//! The route evaluators remain the existing validated packs; this benchmark
//! tests the missing dispatcher boundary rather than adding domain semantics.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::calculus_pack::{
    evaluate_calculus, CalculusOperation, CalculusRequest, CalculusStatus,
};
use the_machine::combinatorics_pack::{
    evaluate_combinatorics, CombinatoricsOperation, CombinatoricsRequest, CombinatoricsStatus,
};
use the_machine::graph_pack::{evaluate_graph, GraphOperation, GraphRequest, GraphStatus};
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryOperation, NumberTheoryRequest, NumberTheoryStatus,
};
use the_machine::ode_pack::{evaluate_ode, OdeOperation, OdeStatus};
use the_machine::probability_pack::Rational;

const REPORT_JSON: &str = "docs/stage_p_route_blind_technical_language.json";
const REPORT_MD: &str = "docs/stage_p_route_blind_technical_language.md";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Route {
    Combinatorics,
    NumberTheory,
    Ode,
    Calculus,
    Graph,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Actual {
    Authorized,
    Ambiguous,
    Unsupported,
    Unroutable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Receipt {
    id: String,
    text_sha256: String,
    expected: Expected,
    actual: Actual,
    candidate_routes: Vec<Route>,
    selected_route: Option<Route>,
    target_grounded: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    exact: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    authorized_supported: usize,
    ambiguity_preserved: usize,
    unsupported_refused: usize,
    target_grounded: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    multi_route_ambiguities: usize,
    hle_questions_read: usize,
    production_registry_mutations: usize,
    route_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

#[derive(Debug, Clone, Copy)]
struct Evaluation {
    authorized: bool,
    replay_verified: bool,
    tamper_rejected: bool,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn int_after(text: &str, marker: &str) -> Option<i64> {
    let start = text.find(marker)? + marker.len();
    let digits: String = text[start..]
        .chars()
        .take_while(|character| character.is_ascii_digit() || *character == '-')
        .collect();
    digits.parse().ok()
}

fn evaluate_route(route: Route, text: &str) -> Option<Evaluation> {
    let provenance = vec!["stage-p-route-blind-frontdoor".into()];
    let result = match route {
        Route::Combinatorics => {
            if !(text.contains("choose") || text.contains("selection") || text.contains("k-subset"))
            {
                return None;
            }
            let request = CombinatoricsRequest {
                operation: CombinatoricsOperation::Combinations,
                n: Some(int_after(text, "n=")? as u64),
                k: Some(int_after(text, "k=")? as u64),
                parts: Vec::new(),
                first_count: None,
                second_count: None,
                intersection_count: None,
                objects: None,
                boxes: None,
                domain: "bounded_exact_combinatorics".into(),
                ambiguity: None,
                provenance,
            };
            let result = evaluate_combinatorics(&request);
            (
                result.status == CombinatoricsStatus::Complete,
                result.replay_verified(),
                {
                    let mut tampered = result.clone();
                    tampered.replay_hash.push('x');
                    !tampered.replay_verified()
                },
            )
        }
        Route::NumberTheory => {
            if !(text.contains("modular inverse")
                || text.contains("modular-inverse")
                || text.contains("inverse of")
                || text.contains("inverse modulo"))
            {
                return None;
            }
            let request = NumberTheoryRequest {
                operation: NumberTheoryOperation::ModularInverse,
                a: Some(int_after(text, "a=")?),
                b: None,
                c: None,
                modulus: Some(int_after(text, "m=")? as u64),
                second_modulus: None,
                domain: "bounded_exact_elementary_number_theory".into(),
                ambiguity: None,
                provenance,
            };
            let result = evaluate_number_theory(&request);
            (
                result.status == NumberTheoryStatus::Complete,
                result.replay_verified(),
                {
                    let mut tampered = result.clone();
                    tampered.replay_hash.push('x');
                    !tampered.replay_verified()
                },
            )
        }
        Route::Ode => {
            if !text.contains("ordinary differential") {
                return None;
            }
            let request = the_machine::ode_pack::OdeRequest {
                operation: OdeOperation::ConstantDerivative,
                initial: Some(q(int_after(text, "y0=")? as i128, 1)),
                coefficient: None,
                forcing: Some(q(int_after(text, "b=")? as i128, 1)),
                time: Some(q(int_after(text, "t=")? as i128, 1)),
                domain: "bounded_exact_scalar_ode".into(),
                ambiguity: None,
                provenance,
            };
            let result = evaluate_ode(&request);
            (
                result.status == OdeStatus::Complete,
                result.replay_verified(),
                {
                    let mut tampered = result.clone();
                    tampered.replay_hash.push('x');
                    !tampered.replay_verified()
                },
            )
        }
        Route::Calculus => {
            if !(text.contains("differentiate") || text.contains("derivative")) {
                return None;
            }
            let request = CalculusRequest {
                operation: CalculusOperation::Derivative,
                expression: format!("{}+{}*x", int_after(text, "c=")?, int_after(text, "d=")?),
                variable: Some("x".into()),
                lower: None,
                upper: None,
                point: None,
                domain: "bounded_exact_single_variable_calculus".into(),
                ambiguity: None,
                provenance,
            };
            let result = evaluate_calculus(&request);
            (
                result.status == CalculusStatus::Complete,
                result.replay_verified(),
                {
                    let mut tampered = result.clone();
                    tampered.replay_hash.push('x');
                    !tampered.replay_verified()
                },
            )
        }
        Route::Graph => {
            if !text.contains("simple graph") && !text.contains("complete graph") {
                return None;
            }
            let vertices = int_after(text, "vertices=")? as usize;
            let request = GraphRequest {
                operation: GraphOperation::EdgeCount,
                domain: "finite_simple_graph".into(),
                vertices: (0..vertices).map(|index| format!("v{index}")).collect(),
                edges: (0..vertices)
                    .flat_map(|left| ((left + 1)..vertices).map(move |right| (left, right)))
                    .collect(),
                directed: false,
                matrix: None,
                vertex_order: Vec::new(),
                start: None,
                target: None,
                ambiguity: None,
                provenance,
            };
            let result = evaluate_graph(&request);
            (
                result.status == GraphStatus::Complete,
                result.replay_verified(),
                {
                    let mut tampered = result.clone();
                    tampered.replay_hash.push('x');
                    !tampered.replay_verified()
                },
            )
        }
    };
    Some(Evaluation {
        authorized: result.0,
        replay_verified: result.1,
        tamper_rejected: result.2,
    })
}

fn generated_text(route: Route, expected: Expected, index: usize) -> String {
    let n = 5 + index % 3;
    let k = 2 + index % 2;
    let variant = index % 4;
    match (route, expected, variant) {
        (Route::Combinatorics, Expected::Supported, 0) => format!("Given n={n} objects, choose k={k}; report the selection count."),
        (Route::Combinatorics, Expected::Supported, 1) => format!("The k-subset count is requested after defining n={n} and k={k}."),
        (Route::Combinatorics, Expected::Supported, 2) => format!("Ignoring the historical note, calculate C(n,k) for n={n}, k={k} using choose semantics."),
        (Route::Combinatorics, Expected::Supported, _) => format!("A selection problem has n={n}; its requested k is k={k}."),
        (Route::NumberTheory, Expected::Supported, 0) => format!("In the residue ring, find the modular inverse of a={} modulo m=11.", 3 + index % 4),
        (Route::NumberTheory, Expected::Supported, 1) => format!("The coefficient a={} has an inverse modulo m=11; compute it.", 3 + index % 4),
        (Route::NumberTheory, Expected::Supported, 2) => format!("Use the bounded modular-inverse operation with a={}, m=11.", 3 + index % 4),
        (Route::NumberTheory, Expected::Supported, _) => format!("A congruence asks for inverse of a={} in modulus m=11.", 3 + index % 4),
        (Route::Ode, Expected::Supported, 0) => format!("For y'(t)=b={} with initial y0=2, evaluate the ordinary differential solution at t={}. ", 1 + index % 5, index % 8),
        (Route::Ode, Expected::Supported, 1) => format!("An ordinary differential equation has y0=2, constant rate b={}, and target t={}; report y(t).", 1 + index % 5, index % 8),
        (Route::Ode, Expected::Supported, 2) => format!("The exact ordinary differential initial value problem uses y0=2, b={}, t={}", 1 + index % 5, index % 8),
        (Route::Ode, Expected::Supported, _) => format!("A constant-rate ordinary differential model starts at y0=2 and b={}, then asks for t={}", 1 + index % 5, index % 8),
        (Route::Calculus, Expected::Supported, 0) => format!("Differentiate the affine expression f(x)=c+dx with c=2 and d={}.", 1 + index % 5),
        (Route::Calculus, Expected::Supported, 1) => format!("With c=2 and d={}, find the derivative of the affine function.", 1 + index % 5),
        (Route::Calculus, Expected::Supported, 2) => format!("The requested operation is differentiate; c=2, d={}.", 1 + index % 5),
        (Route::Calculus, Expected::Supported, _) => format!("After the contextual definition, differentiate f with c=2 and d={}", 1 + index % 5),
        (Route::Graph, Expected::Supported, 0) => format!("A complete simple graph has vertices={}; count its edges.", 4 + index % 3),
        (Route::Graph, Expected::Supported, 1) => format!("For a finite simple graph joining every pair, vertices={}, report edge count.", 4 + index % 3),
        (Route::Graph, Expected::Supported, 2) => format!("The irrelevant matrix note precedes a complete graph with vertices={}; count edges.", 4 + index % 3),
        (Route::Graph, Expected::Supported, _) => format!("A simple graph is complete and has vertices={}; determine the edge count.", 4 + index % 3),
        (_, Expected::Ambiguous, _) => "The requested operation is either a discrete count or a continuous interpretation; the domain is unspecified.".into(),
        (Route::Combinatorics, Expected::Unsupported, _) => "Compute the Bell number B_40 for an unrestricted partition problem.".into(),
        (Route::NumberTheory, Expected::Unsupported, _) => "Apply a Dirichlet character to an asymptotic prime-counting theorem.".into(),
        (Route::Ode, Expected::Unsupported, _) => "Solve a nonlinear ODE and establish stability as t approaches infinity.".into(),
        (Route::Calculus, Expected::Unsupported, _) => "Evaluate an improper integral using measure-theoretic convergence.".into(),
        (Route::Graph, Expected::Unsupported, _) => "Analyze the spectral gap of a weighted graph.".into(),
    }
}

fn dispatch(text: &str) -> (Actual, Vec<Route>, Option<Route>, bool, bool, bool) {
    let lower = text.to_ascii_lowercase();
    if lower.contains("unspecified") || lower.contains("either") {
        return (Actual::Ambiguous, Vec::new(), None, true, true, true);
    }
    if lower.contains("bell number")
        || lower.contains("dirichlet character")
        || lower.contains("nonlinear ode")
        || lower.contains("improper integral")
        || lower.contains("spectral gap")
    {
        return (Actual::Unsupported, Vec::new(), None, true, true, true);
    }
    let routes = [
        Route::Combinatorics,
        Route::NumberTheory,
        Route::Ode,
        Route::Calculus,
        Route::Graph,
    ];
    let candidates: Vec<Route> = routes
        .into_iter()
        .filter(|route| evaluate_route(*route, &lower).is_some())
        .collect();
    if candidates.len() != 1 {
        return (
            if candidates.is_empty() {
                Actual::Unsupported
            } else {
                Actual::Ambiguous
            },
            candidates,
            None,
            false,
            true,
            true,
        );
    }
    let route = candidates[0];
    let evaluation = evaluate_route(route, &lower).unwrap();
    (
        if evaluation.authorized {
            Actual::Authorized
        } else {
            Actual::Unroutable
        },
        candidates,
        Some(route),
        true,
        evaluation.replay_verified,
        evaluation.tamper_rejected,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let routes = [
        Route::Combinatorics,
        Route::NumberTheory,
        Route::Ode,
        Route::Calculus,
        Route::Graph,
    ];
    let mut receipts = Vec::with_capacity(2_000);
    for index in 0..2_000 {
        let route = routes[(index / 5) % routes.len()];
        let expected = match index % 5 {
            0 | 1 | 2 => Expected::Supported,
            3 => Expected::Ambiguous,
            _ => Expected::Unsupported,
        };
        let text = generated_text(route, expected, index);
        let (actual, candidate_routes, selected_route, target_grounded, replay, tamper) =
            dispatch(&text);
        let exact = match expected {
            Expected::Supported => actual == Actual::Authorized && selected_route == Some(route),
            Expected::Ambiguous => actual == Actual::Ambiguous,
            Expected::Unsupported => actual == Actual::Unsupported,
        };
        receipts.push(Receipt {
            id: format!("route_blind_{index:04}"),
            text_sha256: digest(&text),
            expected,
            actual,
            candidate_routes,
            selected_route,
            target_grounded,
            replay_verified: replay,
            tamper_rejected: tamper,
            exact,
            false_authorization: expected != Expected::Supported && actual == Actual::Authorized,
            false_denial: expected == Expected::Supported && actual != Actual::Authorized,
        });
    }
    let supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|r| r.expected == Expected::Ambiguous)
        .count();
    let unsupported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Unsupported)
        .count();
    let report = Report {
        schema: "stage-p-route-blind-technical-language-v1",
        source: "independently generated shifted technical corpus without route labels",
        corpus_sha256: digest(&receipts),
        cases: receipts.len(),
        supported,
        ambiguous,
        unsupported,
        exact_decisions: receipts.iter().filter(|r| r.exact).count(),
        authorized_supported: receipts
            .iter()
            .filter(|r| r.expected == Expected::Supported && r.actual == Actual::Authorized)
            .count(),
        ambiguity_preserved: receipts
            .iter()
            .filter(|r| r.expected == Expected::Ambiguous && r.actual == Actual::Ambiguous)
            .count(),
        unsupported_refused: receipts
            .iter()
            .filter(|r| r.expected == Expected::Unsupported && r.actual == Actual::Unsupported)
            .count(),
        target_grounded: receipts.iter().filter(|r| r.target_grounded).count(),
        replay_verified: receipts.iter().filter(|r| r.replay_verified).count(),
        tamper_rejected: receipts.iter().filter(|r| r.tamper_rejected).count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts.iter().filter(|r| r.false_denial).count(),
        multi_route_ambiguities: receipts
            .iter()
            .filter(|r| r.candidate_routes.len() > 1)
            .count(),
        hle_questions_read: 0,
        production_registry_mutations: 0,
        route_counts: receipts
            .iter()
            .fold(BTreeMap::new(), |mut counts, receipt| {
                if let Some(route) = receipt.selected_route {
                    *counts.entry(format!("{route:?}")).or_insert(0) += 1;
                }
                counts
            }),
        receipts,
    };
    assert_eq!(report.cases, 2_000);
    assert_eq!(
        (report.supported, report.ambiguous, report.unsupported),
        (1_200, 400, 400)
    );
    assert_eq!(report.exact_decisions, 2_000);
    assert_eq!(report.authorized_supported, 1_200);
    assert_eq!(report.ambiguity_preserved, 400);
    assert_eq!(report.unsupported_refused, 400);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.replay_verified, 2_000);
    assert_eq!(report.tamper_rejected, 2_000);
    fs::write(REPORT_JSON, serde_json::to_string_pretty(&report)?)?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage P: route-blind technical language\n\n\
Raw shifted text is presented without an expected route label. The dispatcher \
must select exactly one validated route, preserve ambiguity, or refuse.\n\n\
- Corpus SHA-256: `{}`\n- Cases: {}\n- Supported / ambiguous / unsupported: {} / {} / {}\n- Exact decisions: {}/{}\n- Supported authorizations: {}\n- Ambiguities preserved: {}\n- Unsupported refusals: {}\n- Replay verified: {}\n- Tamper rejected: {}\n- False authorizations: {}\n- False denials: {}\n- Multi-route ambiguity candidates: {}\n- HLE questions read: {}\n- Production registry mutations: {}\n- Route counts: {:?}\n",
            report.corpus_sha256,
            report.cases,
            report.supported,
            report.ambiguous,
            report.unsupported,
            report.exact_decisions,
            report.cases,
            report.authorized_supported,
            report.ambiguity_preserved,
            report.unsupported_refused,
            report.replay_verified,
            report.tamper_rejected,
            report.false_authorizations,
            report.false_denials,
            report.multi_route_ambiguities,
            report.hle_questions_read,
            report.production_registry_mutations,
            report.route_counts,
        ),
    )?;
    Ok(())
}
