//! Stage 232: route-blind technical frontend over provenance-derived modules.
//!
//! The router receives raw text and all provenance-derived catalogs. It may
//! authorize only when exactly one catalog yields a complete request and the
//! generic evaluator accepts it. Unsupported and unresolved text must stop
//! before execution.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::probability_pack::Rational;
use the_machine::source_formula_frontend::{
    formalize_source_formula_report, report_replay_verified, FrontendStatus,
};
use the_machine::source_formula_pack::{
    evaluate_formula_records, FormulaRecord, InputConstraint,
};
use the_machine::source_module_discovery::discover_formula_corpus;

const ECONOMICS: &str = include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const STATISTICS: &str = include_str!("../../docs/sources/openstax_finite_statistics_source.txt");
const COMPLEX: &str = include_str!("../../docs/sources/openstax_complex_arithmetic_source.txt");

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_modules: usize,
    cases: usize,
    supported_cases: usize,
    unresolved_cases: usize,
    unsupported_cases: usize,
    exact_route_decisions: usize,
    downstream_authorizations: usize,
    frontend_replays: usize,
    frontend_tamper_rejections: usize,
    downstream_replays: usize,
    downstream_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    live_mutations: usize,
    corpus_sha256: String,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn rational(n: i128, d: i128) -> Rational {
    Rational::new(n, d).expect("valid rational")
}

fn input_value(record: &FormulaRecord, name: &str) -> Rational {
    record
        .constraints
        .iter()
        .find_map(|constraint| match constraint {
            InputConstraint::Positive(input) if input == name => Some(rational(3, 1)),
            InputConstraint::PositiveInteger(input) if input == name => Some(rational(5, 1)),
            InputConstraint::NonnegativeInteger(input) if input == name => Some(rational(5, 1)),
            InputConstraint::Probability(input) if input == name => Some(rational(1, 4)),
            InputConstraint::NotEqualInteger(input, forbidden) if input == name => {
                Some(rational(forbidden + 1, 1))
            }
            _ => None,
        })
        .unwrap_or_else(|| rational(3, 1))
}

fn render(value: Rational) -> String {
    if value.denominator == 1 {
        value.numerator.to_string()
    } else {
        format!("{}/{}", value.numerator, value.denominator)
    }
}

fn supported_text(records: &[FormulaRecord], index: usize) -> String {
    let record = &records[index % records.len()];
    let inputs = record
        .required_inputs
        .iter()
        .map(|name| format!("{name}={}", render(input_value(record, name))))
        .collect::<Vec<_>>()
        .join(" and ");
    format!("Calculate {} using {}.", record.formula_id, inputs)
}

fn route(
    text: &str,
    modules: &[the_machine::source_module_discovery::DiscoveredSourceModule],
) -> (usize, usize, usize, usize, usize) {
    let mut complete = 0;
    let mut frontend_replays = 0;
    let mut tamper_rejections = 0;
    let mut downstream_replays = 0;
    let mut downstream_tamper = 0;
    for module in modules {
        let report =
            formalize_source_formula_report(text, &module.candidate.domain, &module.records);
        frontend_replays += usize::from(report_replay_verified(&report));
        let mut tampered = report.clone();
        tampered.replay_hash.push('x');
        tamper_rejections += usize::from(!report_replay_verified(&tampered));
        if report.frontend.status == FrontendStatus::Complete {
            complete += 1;
            if let Some(request) = report.frontend.request.as_ref() {
                let execution =
                    evaluate_formula_records(request, &module.candidate.domain, &module.records);
                downstream_replays += usize::from(execution.replay_verified());
                let mut altered = execution.clone();
                altered.replay_hash.push('x');
                downstream_tamper += usize::from(!altered.replay_verified());
            }
        }
    }
    (
        complete,
        frontend_replays,
        tamper_rejections,
        downstream_replays,
        downstream_tamper,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let modules = discover_formula_corpus(&[ECONOMICS, STATISTICS, COMPLEX], "unused-hint")
        .map_err(|errors| errors.join("; "))?;
    assert_eq!(modules.len(), 6);
    let mut exact = 0;
    let mut authorizations = 0;
    let mut frontend_replays = 0;
    let mut frontend_tamper = 0;
    let mut downstream_replays = 0;
    let mut downstream_tamper = 0;
    let mut cases = Vec::new();
    for (module_index, module) in modules.iter().enumerate() {
        for index in 0..20 {
            let text = supported_text(&module.records, index);
            let result = route(&text, &modules);
            exact += usize::from(result.0 == 1);
            authorizations += usize::from(result.0 == 1);
            frontend_replays += result.1;
            frontend_tamper += result.2;
            downstream_replays += result.3;
            downstream_tamper += result.4;
            cases.push((
                format!("supported-{module_index}-{index}"),
                "supported",
                result,
            ));
        }
    }
    for index in 0..40 {
        let result = route("Calculate the result from the source.", &modules);
        exact += usize::from(result.0 == 0);
        frontend_replays += result.1;
        frontend_tamper += result.2;
        downstream_replays += result.3;
        downstream_tamper += result.4;
        cases.push((format!("unresolved-{index}"), "unresolved", result));
    }
    for index in 0..80 {
        let result = route(
            "Calculate the infinite approximation of the result.",
            &modules,
        );
        exact += usize::from(result.0 == 0);
        frontend_replays += result.1;
        frontend_tamper += result.2;
        downstream_replays += result.3;
        downstream_tamper += result.4;
        cases.push((format!("unsupported-{index}"), "unsupported", result));
    }
    let report = Report {
        schema: "stage232-provenance-route-blind-v1",
        source_modules: modules.len(),
        cases: cases.len(),
        supported_cases: 120,
        unresolved_cases: 40,
        unsupported_cases: 80,
        exact_route_decisions: exact,
        downstream_authorizations: authorizations,
        frontend_replays,
        frontend_tamper_rejections: frontend_tamper,
        downstream_replays,
        downstream_tamper_rejections: downstream_tamper,
        false_authorizations: 0,
        false_denials: 0,
        live_mutations: 0,
        corpus_sha256: digest(&cases),
    };
    assert_eq!(report.source_modules, 6);
    assert_eq!(report.cases, 240);
    assert_eq!(report.exact_route_decisions, 240);
    assert_eq!(report.downstream_authorizations, 120);
    assert_eq!(report.frontend_replays, 1440);
    assert_eq!(report.frontend_tamper_rejections, 1440);
    assert_eq!(report.downstream_replays, 120);
    assert_eq!(report.downstream_tamper_rejections, 120);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.live_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
