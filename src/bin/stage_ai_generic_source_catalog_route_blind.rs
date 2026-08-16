//! Stage AI: route-blind composition of independently sourced formula catalogs.
//!
//! Each catalog is parsed as data and reaches the same generic frontend and
//! expression evaluator.  The dispatcher never names a subject or formula;
//! it authorizes a route only when exactly one catalog has a complete,
//! replayable frontend result.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_frontend::{
    formalize_formula_text, FormulaFrontendResult, FormulaFrontendStatus,
};
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaRecord, FormulaResult,
    InputConstraint,
};

const ECONOMICS: &str = include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const STATISTICS: &str = include_str!("../../docs/sources/openstax_finite_statistics_source.txt");
const REGRESSION: &str = include_str!("../../docs/sources/openstax_finite_regression_source.txt");
const COMPLEX: &str = include_str!("../../docs/sources/openstax_complex_arithmetic_source.txt");

#[derive(Clone)]
struct Catalog {
    name: &'static str,
    domain: &'static str,
    records: Vec<FormulaRecord>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    selected_catalog: Option<String>,
    frontend_statuses: BTreeMap<String, FormulaFrontendStatus>,
    frontend_exact: bool,
    frontend_replay: bool,
    frontend_tamper_rejected: bool,
    downstream_status: Option<String>,
    downstream_replay: bool,
    downstream_tamper_rejected: bool,
    exact: bool,
    false_authorization: bool,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    source_sha256: BTreeMap<String, String>,
    catalog_record_counts: BTreeMap<String, usize>,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_route_decisions: usize,
    frontend_replays: usize,
    frontend_tamper_rejections: usize,
    downstream_artifacts: usize,
    downstream_exact: usize,
    downstream_replays: usize,
    downstream_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    live_mutations: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn rational(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("benchmark rational is valid")
}

fn input_value(record: &FormulaRecord, input: &str) -> Rational {
    for constraint in &record.constraints {
        match constraint {
            InputConstraint::Probability(name) if name == input => return rational(1, 4),
            InputConstraint::PositiveInteger(name) if name == input => return rational(5, 1),
            InputConstraint::NonnegativeInteger(name) if name == input => return rational(5, 1),
            InputConstraint::NotEqualInteger(name, value) if name == input => {
                return rational(if *value == 2 { 3 } else { 2 }, 1)
            }
            InputConstraint::Positive(name) if name == input => return rational(3, 1),
            _ => {}
        }
    }
    rational(3, 1)
}

fn source_text(record: &FormulaRecord, index: usize) -> String {
    let inputs = record
        .required_inputs
        .iter()
        .map(|input| {
            let value = input_value(record, input);
            let rendered = if value.denominator == 1 {
                value.numerator.to_string()
            } else {
                format!("{}/{}", value.numerator, value.denominator)
            };
            format!("{input}={rendered}")
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "Compute {} with {}. case {index}",
        record.formula_id, inputs
    )
}

fn frontend_for(text: &str, catalog: &Catalog) -> FormulaFrontendResult {
    formalize_formula_text(text, catalog.domain, &catalog.records)
}

fn tamper_frontend(result: &FormulaFrontendResult) -> bool {
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    !tampered.replay_verified()
}

fn tamper_downstream(result: &FormulaResult) -> bool {
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    !tampered.replay_verified()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalogs = vec![
        Catalog {
            name: "economics",
            domain: "source_derived_bounded_economics",
            records: extract_formula_records(ECONOMICS).map_err(|e| format!("economics: {e:?}"))?,
        },
        Catalog {
            name: "statistics",
            domain: "source_derived_finite_statistics",
            records: extract_formula_records(STATISTICS)
                .map_err(|e| format!("statistics: {e:?}"))?,
        },
        Catalog {
            name: "regression",
            domain: "source_derived_finite_regression",
            records: extract_formula_records(REGRESSION)
                .map_err(|e| format!("regression: {e:?}"))?,
        },
        Catalog {
            name: "complex_arithmetic",
            domain: "source_derived_rectangular_complex_arithmetic",
            records: extract_formula_records(COMPLEX).map_err(|e| format!("complex: {e:?}"))?,
        },
    ];
    let source_documents = [
        ("economics", ECONOMICS),
        ("statistics", STATISTICS),
        ("regression", REGRESSION),
        ("complex_arithmetic", COMPLEX),
    ]
    .into_iter()
    .map(|(name, source)| (name.to_owned(), digest(source)))
    .collect();
    let catalog_record_counts = catalogs
        .iter()
        .map(|catalog| (catalog.name.to_owned(), catalog.records.len()))
        .collect();

    let mut cases = Vec::new();
    for (catalog_index, catalog) in catalogs.iter().enumerate() {
        for index in 0..30 {
            let record = &catalog.records[index % catalog.records.len()];
            cases.push((
                format!("supported-{catalog_index}-{index}"),
                source_text(record, index),
                Expected::Supported,
            ));
        }
    }
    // Two known formula identifiers in one text deliberately preserve an
    // ambiguity instead of selecting the first matching catalog record.
    for index in 0..40 {
        let catalog = &catalogs[index % catalogs.len()];
        let left = &catalog.records[0];
        let right = &catalog.records[1 % catalog.records.len()];
        let values = left
            .required_inputs
            .iter()
            .map(|input| {
                let value = input_value(left, input);
                let rendered = if value.denominator == 1 {
                    value.numerator.to_string()
                } else {
                    format!("{}/{}", value.numerator, value.denominator)
                };
                format!("{input}={rendered}")
            })
            .collect::<Vec<_>>()
            .join(" ");
        cases.push((
            format!("ambiguous-{index}"),
            format!(
                "Compute {} and {} with {values}.",
                left.formula_id, right.formula_id
            ),
            Expected::Ambiguous,
        ));
    }
    for index in 0..80 {
        let text = if index % 2 == 0 {
            format!("Compute an unknown source formula with value={index}.")
        } else {
            format!(
                "Compute a continuous infinite differential optimization expression with x={index}."
            )
        };
        cases.push((format!("refused-{index}"), text, Expected::Refused));
    }
    assert_eq!(cases.len(), 240);

    let mut receipts = Vec::with_capacity(cases.len());
    let mut live_mutations = 0;
    for (id, text, expected) in cases {
        let mut results = Vec::new();
        let mut statuses = BTreeMap::new();
        for catalog in &catalogs {
            let result = frontend_for(&text, catalog);
            statuses.insert(catalog.name.to_owned(), result.status);
            results.push((catalog, result));
        }
        let complete: Vec<_> = results
            .iter()
            .filter(|(_, result)| result.status == FormulaFrontendStatus::Complete)
            .collect();
        let selected = if complete.len() == 1 {
            Some(complete[0])
        } else {
            None
        };
        let frontend_replay = results.iter().all(|(_, result)| result.replay_verified());
        let frontend_tamper = results.iter().all(|(_, result)| tamper_frontend(result));
        let mut downstream_status = None;
        let mut downstream_replay = false;
        let mut downstream_tamper = false;
        let mut downstream_exact = true;
        let mut selected_name = None;
        if let Some((catalog, frontend)) = selected {
            selected_name = Some(catalog.name.to_owned());
            let request = frontend
                .request
                .as_ref()
                .expect("complete frontend request");
            let result = evaluate_formula_records(request, catalog.domain, &catalog.records);
            downstream_status = Some(format!("{:?}", result.status));
            downstream_replay = result.replay_verified();
            downstream_tamper = tamper_downstream(&result);
            downstream_exact = format!("{:?}", result.status) == "Complete";
        }
        let actual = if selected.is_some() {
            Expected::Supported
        } else if results
            .iter()
            .any(|(_, result)| result.status == FormulaFrontendStatus::Ambiguous)
        {
            Expected::Ambiguous
        } else {
            Expected::Refused
        };
        let exact = actual == expected && (actual != Expected::Supported || downstream_exact);
        let false_authorization = expected != Expected::Supported && actual == Expected::Supported;
        if false_authorization {
            live_mutations += 1;
        }
        receipts.push(Receipt {
            id,
            expected,
            selected_catalog: selected_name,
            frontend_statuses: statuses,
            frontend_exact: actual == expected || expected == Expected::Supported,
            frontend_replay,
            frontend_tamper_rejected: frontend_tamper,
            downstream_status,
            downstream_replay,
            downstream_tamper_rejected: downstream_tamper,
            exact,
            false_authorization,
        });
    }
    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|r| r.expected == Expected::Ambiguous)
        .count();
    let refused = receipts
        .iter()
        .filter(|r| r.expected == Expected::Refused)
        .count();
    let exact_route_decisions = receipts.iter().filter(|r| r.exact).count();
    let frontend_replays = receipts.iter().filter(|r| r.frontend_replay).count();
    let frontend_tamper_rejections = receipts
        .iter()
        .filter(|r| r.frontend_tamper_rejected)
        .count();
    let downstream_artifacts = receipts
        .iter()
        .filter(|r| r.selected_catalog.is_some())
        .count();
    let downstream_exact = receipts
        .iter()
        .filter(|r| r.downstream_status.as_deref() == Some("Complete"))
        .count();
    let downstream_replays = receipts.iter().filter(|r| r.downstream_replay).count();
    let downstream_tamper_rejections = receipts
        .iter()
        .filter(|r| r.downstream_tamper_rejected)
        .count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && !r.exact)
        .count();
    assert_eq!(supported, 120);
    assert_eq!(ambiguous, 40);
    assert_eq!(refused, 80);
    assert_eq!(exact_route_decisions, cases);
    assert_eq!(frontend_replays, cases);
    assert_eq!(frontend_tamper_rejections, cases);
    assert_eq!(downstream_artifacts, supported);
    assert_eq!(downstream_exact, supported);
    assert_eq!(downstream_replays, downstream_artifacts);
    assert_eq!(downstream_tamper_rejections, downstream_artifacts);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    assert_eq!(live_mutations, 0);
    let report = Report {
        schema: "stage-ai-generic-source-catalog-route-blind-v1",
        source_sha256: source_documents,
        catalog_record_counts,
        cases,
        supported,
        ambiguous,
        refused,
        exact_route_decisions,
        frontend_replays,
        frontend_tamper_rejections,
        downstream_artifacts,
        downstream_exact,
        downstream_replays,
        downstream_tamper_rejections,
        false_authorizations,
        false_denials,
        live_mutations,
        receipts,
    };
    std::fs::write(
        "docs/stage_ai_generic_source_catalog_route_blind.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    std::fs::write(
        "docs/stage_ai_generic_source_catalog_route_blind.md",
        format!(
            "# Stage AI — generic source catalog route-blind checkpoint\n\n\
Four independently sourced declarative catalogs were dispatched through the\n\
same domain-agnostic frontend and expression evaluator. The route was selected\n\
only when exactly one catalog produced a complete typed request.\n\n\
Results: {cases} cases; {supported} supported, {ambiguous} ambiguous, {refused} refused;\n\
{exact_route_decisions}/{cases} exact route decisions; {downstream_artifacts}/{supported} downstream\n\
artifacts complete; {frontend_replays}/{cases} frontend replays;\n\
{downstream_replays}/{downstream_artifacts} downstream replays; {frontend_tamper_rejections}/{cases} frontend\n\
tamper rejections; {downstream_tamper_rejections}/{downstream_artifacts} downstream tamper rejections;\n\
zero false authorizations and zero live mutations. HLE remains untouched.\n"
        ),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
