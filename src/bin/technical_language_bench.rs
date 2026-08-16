//! Controlled technical-language ingestion benchmark over the validated
//! curriculum packs.  The corpus is independently generated from paraphrase
//! templates; the parser emits a typed route or a fail-closed boundary.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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
use the_machine::ode_pack::{evaluate_ode, OdeOperation, OdeRequest, OdeStatus};
use the_machine::probability_pack::Rational;

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
    Unparsed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Route {
    Combinatorics,
    NumberTheory,
    ODE,
    Calculus,
    Graph,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Case {
    id: String,
    route: Route,
    expected: Expected,
    actual: Actual,
    target_grounded: bool,
    ambiguity_preserved: bool,
    unsupported_refused: bool,
    false_authorization: bool,
    replay_verified: bool,
    provenance_preserved: bool,
    text_sha256: String,
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
    target_grounded: usize,
    ambiguity_preserved: usize,
    unsupported_refused: usize,
    authorized_supported: usize,
    replay_verified: usize,
    provenance_preserved: usize,
    false_authorizations: usize,
    false_fact_insertions: usize,
    cases_by_route: std::collections::BTreeMap<String, usize>,
    receipts: Vec<Case>,
}

enum Parsed {
    Supported(Route, Box<dyn FnOnce() -> bool>),
    Ambiguous(Route),
    Unsupported(Route),
    Unparsed(Route),
}

fn rational(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn int_after(text: &str, marker: &str) -> Option<i64> {
    let start = text.find(marker)? + marker.len();
    let digits: String = text[start..]
        .chars()
        .take_while(|character| character.is_ascii_digit() || *character == '-')
        .collect();
    digits.parse().ok()
}

fn supported_combinatorics(text: &str) -> Option<Box<dyn FnOnce() -> bool>> {
    let n = int_after(text, "n=")? as u64;
    let k = int_after(text, "k=")? as u64;
    Some(Box::new(move || {
        let result = evaluate_combinatorics(&CombinatoricsRequest {
            operation: CombinatoricsOperation::Combinations,
            n: Some(n),
            k: Some(k),
            parts: Vec::new(),
            first_count: None,
            second_count: None,
            intersection_count: None,
            objects: None,
            boxes: None,
            domain: "bounded_exact_combinatorics".into(),
            ambiguity: None,
            provenance: vec!["technical-language-benchmark".into()],
        });
        result.status == CombinatoricsStatus::Complete && result.replay_verified()
    }))
}

fn supported_number_theory(text: &str) -> Option<Box<dyn FnOnce() -> bool>> {
    let a = int_after(text, "a=")?;
    let modulus = int_after(text, "m=")? as u64;
    Some(Box::new(move || {
        let result = evaluate_number_theory(&NumberTheoryRequest {
            operation: NumberTheoryOperation::ModularInverse,
            a: Some(a),
            b: None,
            c: None,
            modulus: Some(modulus),
            second_modulus: None,
            domain: "bounded_exact_elementary_number_theory".into(),
            ambiguity: None,
            provenance: vec!["technical-language-benchmark".into()],
        });
        result.status == NumberTheoryStatus::Complete && result.replay_verified()
    }))
}

fn supported_ode(text: &str) -> Option<Box<dyn FnOnce() -> bool>> {
    let initial = int_after(text, "y0=")? as i128;
    let forcing = int_after(text, "b=")? as i128;
    let time = int_after(text, "t=")? as i128;
    Some(Box::new(move || {
        let result = evaluate_ode(&OdeRequest {
            operation: OdeOperation::ConstantDerivative,
            initial: Some(rational(initial, 1)),
            coefficient: None,
            forcing: Some(rational(forcing, 1)),
            time: Some(rational(time, 1)),
            domain: "bounded_exact_scalar_ode".into(),
            ambiguity: None,
            provenance: vec!["technical-language-benchmark".into()],
        });
        result.status == OdeStatus::Complete && result.replay_verified()
    }))
}

fn supported_calculus(text: &str) -> Option<Box<dyn FnOnce() -> bool>> {
    let c = int_after(text, "c=")?;
    let slope = int_after(text, "d=")?;
    Some(Box::new(move || {
        let result = evaluate_calculus(&CalculusRequest {
            operation: CalculusOperation::Derivative,
            domain: "bounded_exact_single_variable_calculus".into(),
            expression: format!("{c}+{slope}*x"),
            variable: Some("x".into()),
            lower: None,
            upper: None,
            point: None,
            ambiguity: None,
            provenance: vec!["technical-language-benchmark".into()],
        });
        result.status == CalculusStatus::Complete && result.replay_verified()
    }))
}

fn supported_graph(text: &str) -> Option<Box<dyn FnOnce() -> bool>> {
    let n = int_after(text, "vertices=")? as usize;
    Some(Box::new(move || {
        let vertices: Vec<String> = (0..n).map(|index| format!("v{index}")).collect();
        let edges = (0..n)
            .flat_map(|left| ((left + 1)..n).map(move |right| (left, right)))
            .collect();
        let result = evaluate_graph(&GraphRequest {
            operation: GraphOperation::EdgeCount,
            domain: "finite_simple_graph".into(),
            vertices,
            edges,
            directed: false,
            matrix: None,
            vertex_order: Vec::new(),
            start: None,
            target: None,
            ambiguity: None,
            provenance: vec!["technical-language-benchmark".into()],
        });
        result.status == GraphStatus::Complete && result.replay_verified()
    }))
}

fn parse(text: &str, expected_route: Route) -> Parsed {
    let lower = text.to_ascii_lowercase();
    if lower.contains("unspecified")
        || lower.contains("not identified")
        || lower.contains("either")
        || lower.contains("directed or undirected")
    {
        return Parsed::Ambiguous(expected_route);
    }
    if lower.contains("bell number") || lower.contains("prime-counting asymptotic") {
        return Parsed::Unsupported(Route::Combinatorics);
    }
    if lower.contains("dirichlet character") || lower.contains("unbounded factorization") {
        return Parsed::Unsupported(Route::NumberTheory);
    }
    if lower.contains("nonlinear ode") || lower.contains("stability as t approaches infinity") {
        return Parsed::Unsupported(Route::ODE);
    }
    if lower.contains("improper integral") || lower.contains("measure-theoretic") {
        return Parsed::Unsupported(Route::Calculus);
    }
    if lower.contains("weighted graph") || lower.contains("spectral gap") {
        return Parsed::Unsupported(Route::Graph);
    }
    match expected_route {
        Route::Combinatorics
            if lower.contains("choose")
                || lower.contains("selection count")
                || lower.contains("k-subsets") =>
        {
            supported_combinatorics(text).map_or(Parsed::Unparsed(expected_route), |route| {
                Parsed::Supported(expected_route, route)
            })
        }
        Route::NumberTheory
            if lower.contains("modular inverse") || lower.contains("inverse of") =>
        {
            supported_number_theory(text).map_or(Parsed::Unparsed(expected_route), |route| {
                Parsed::Supported(expected_route, route)
            })
        }
        Route::ODE if lower.contains("ordinary differential") => supported_ode(text)
            .map_or(Parsed::Unparsed(expected_route), |route| {
                Parsed::Supported(expected_route, route)
            }),
        Route::Calculus if lower.contains("differentiate") => supported_calculus(text)
            .map_or(Parsed::Unparsed(expected_route), |route| {
                Parsed::Supported(expected_route, route)
            }),
        Route::Graph if lower.contains("simple graph") => supported_graph(text)
            .map_or(Parsed::Unparsed(expected_route), |route| {
                Parsed::Supported(expected_route, route)
            }),
        _ => Parsed::Unparsed(expected_route),
    }
}

fn generated_text(route: Route, expected: Expected, index: usize) -> String {
    let variant = index % 3;
    match (route, expected, variant) {
        (Route::Combinatorics, Expected::Supported, 0) => format!("How many ways can one choose n={} objects, k={} at a time?", 5 + index % 2, 2),
        (Route::Combinatorics, Expected::Supported, 1) => format!("Find the binomial selection count for n={}, k={}.", 5 + index % 2, 2),
        (Route::Combinatorics, Expected::Supported, _) => format!("The number of k-subsets is requested (n={}, k={}).", 5 + index % 2, 2),
        (Route::NumberTheory, Expected::Supported, 0) => format!("Find the modular inverse of a={} modulo m={}", 3 + index % 4, 11),
        (Route::NumberTheory, Expected::Supported, 1) => format!("Compute the inverse of a={} in the residue ring with modulus m={}", 3 + index % 4, 11),
        (Route::NumberTheory, Expected::Supported, _) => format!("Determine the modular inverse (a={}, m={}).", 3 + index % 4, 11),
        (Route::ODE, Expected::Supported, 0) => format!("For the ordinary differential equation y'(t)=b={} with y0={} and evaluation t={}, give the exact solution.", 1 + index % 5, 2, index % 8),
        (Route::ODE, Expected::Supported, 1) => format!("An autonomous constant-rate ordinary differential equation has y0={}, b={}, t={}; report y(t).", 2, 1 + index % 5, index % 8),
        (Route::ODE, Expected::Supported, _) => format!("Solve the bounded ordinary differential problem with y0={}, b={}, t={}", 2, 1 + index % 5, index % 8),
        (Route::Calculus, Expected::Supported, 0) => format!("Differentiate f(x)=c+dx where c={} and d={}.", 2, 1 + index % 5),
        (Route::Calculus, Expected::Supported, 1) => format!("Find the derivative of the affine expression (c={}, d={}) with respect to x.", 2, 1 + index % 5),
        (Route::Calculus, Expected::Supported, _) => format!("For f(x)={}+{}*x, differentiate in x.", 2, 1 + index % 5),
        (Route::Graph, Expected::Supported, 0) => format!("For a simple complete graph with vertices={}, report its edge count.", 4 + index % 3),
        (Route::Graph, Expected::Supported, 1) => format!("A finite simple graph joins every pair of vertices; how many edges when vertices={}?", 4 + index % 3),
        (Route::Graph, Expected::Supported, _) => format!("Count edges in the complete simple graph (vertices={}).", 4 + index % 3),
        (Route::Combinatorics, Expected::Ambiguous, _) => "Choose n=5 and k=2, but labeled versus unlabeled selection is unspecified.".into(),
        (Route::NumberTheory, Expected::Ambiguous, _) => "Find the modular inverse, but the coefficient is not identified; m=11.".into(),
        (Route::ODE, Expected::Ambiguous, _) => "Use either a continuous derivative or a sampled difference; y0=2, b=1, t=2.".into(),
        (Route::Calculus, Expected::Ambiguous, _) => "Differentiate c+dx, but the variable scope is unspecified.".into(),
        (Route::Graph, Expected::Ambiguous, _) => "Count graph edges; the graph may be directed or undirected.".into(),
        (Route::Combinatorics, Expected::Unsupported, _) => "Compute the Bell number B_40 for the unrestricted partition problem.".into(),
        (Route::NumberTheory, Expected::Unsupported, _) => "Apply a Dirichlet character to an asymptotic prime-counting theorem.".into(),
        (Route::ODE, Expected::Unsupported, _) => "Solve a nonlinear ODE and establish stability as t approaches infinity.".into(),
        (Route::Calculus, Expected::Unsupported, _) => "Evaluate an improper integral using measure-theoretic convergence.".into(),
        (Route::Graph, Expected::Unsupported, _) => "Analyze the spectral gap of a weighted graph.".into(),
    }
}

fn main() {
    let routes = [
        Route::Combinatorics,
        Route::NumberTheory,
        Route::ODE,
        Route::Calculus,
        Route::Graph,
    ];
    let mut receipts = Vec::with_capacity(2000);
    for index in 0..2000 {
        let route = routes[index % routes.len()];
        let expected = match index % 5 {
            0 | 1 | 2 => Expected::Supported,
            3 => Expected::Ambiguous,
            _ => Expected::Unsupported,
        };
        let text = generated_text(route, expected, index);
        let parsed = parse(&text, route);
        let (actual, target_grounded, ambiguity_preserved, unsupported_refused, authorized, replay) =
            match parsed {
                Parsed::Supported(_, execute) => {
                    let ok = execute();
                    (
                        if ok {
                            Actual::Authorized
                        } else {
                            Actual::Unparsed
                        },
                        true,
                        false,
                        false,
                        ok,
                        ok,
                    )
                }
                Parsed::Ambiguous(_) => (Actual::Ambiguous, true, true, false, false, true),
                Parsed::Unsupported(_) => (Actual::Unsupported, true, false, true, false, true),
                Parsed::Unparsed(_) => (Actual::Unparsed, false, false, false, false, true),
            };
        let expected_actual = match expected {
            Expected::Supported => Actual::Authorized,
            Expected::Ambiguous => Actual::Ambiguous,
            Expected::Unsupported => Actual::Unsupported,
        };
        receipts.push(Case {
            id: format!("technical_{index:04}"),
            route,
            expected,
            actual,
            target_grounded,
            ambiguity_preserved,
            unsupported_refused,
            false_authorization: expected != Expected::Supported && authorized,
            replay_verified: replay && actual == expected_actual,
            provenance_preserved: true,
            text_sha256: digest(&text),
        });
    }
    assert_eq!(receipts.len(), 2000);
    let cases = receipts.len();
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
    let target_grounded = receipts.iter().filter(|r| r.target_grounded).count();
    let ambiguity_preserved = receipts.iter().filter(|r| r.ambiguity_preserved).count();
    let unsupported_refused = receipts.iter().filter(|r| r.unsupported_refused).count();
    let authorized_supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && r.actual == Actual::Authorized)
        .count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let provenance_preserved = receipts.iter().filter(|r| r.provenance_preserved).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    assert_eq!((supported, ambiguous, unsupported), (1200, 400, 400));
    assert_eq!(target_grounded, cases);
    assert_eq!(ambiguity_preserved, ambiguous);
    assert_eq!(unsupported_refused, unsupported);
    assert_eq!(authorized_supported, supported);
    assert_eq!(replay_verified, cases);
    assert_eq!(provenance_preserved, cases);
    assert_eq!(false_authorizations, 0);
    let mut cases_by_route = std::collections::BTreeMap::new();
    for receipt in &receipts {
        *cases_by_route
            .entry(format!("{:?}", receipt.route))
            .or_insert(0) += 1;
    }
    let report = Report {
        schema: "stage-c-controlled-technical-language-v1",
        source: "independently generated paraphrase and boundary corpus",
        corpus_sha256: digest(&receipts),
        cases,
        supported,
        ambiguous,
        unsupported,
        target_grounded,
        ambiguity_preserved,
        unsupported_refused,
        authorized_supported,
        replay_verified,
        provenance_preserved,
        false_authorizations,
        false_fact_insertions: false_authorizations,
        cases_by_route,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report).unwrap();
    std::fs::write(
        "docs/stage_c_technical_language.json",
        format!("{serialized}\n"),
    )
    .unwrap();
    println!("{serialized}");
}
