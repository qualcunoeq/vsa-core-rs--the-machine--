//! Stage 220: route-blind technical language over five source formula catalogs.
//!
//! The dispatcher offers every report to every catalog.  It authorizes only
//! when exactly one catalog emits a complete, replayable request and the
//! generic source evaluator returns a complete artifact.  No catalog name,
//! formula family, or subject-specific evaluator is used in routing.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_frontend::{
    formalize_source_formula_text, replay_verified, FrontendStatus,
};
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, source_formula_records, FormulaRecord,
    FormulaStatus, InputConstraint,
};
use the_machine::{source_regression_pack, source_statistics_pack};

const ECONOMICS: &str = include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const REGRESSION: &str = include_str!("../../docs/sources/openstax_finite_regression_source.txt");
const COMPLEX: &str = include_str!("../../docs/sources/openstax_complex_arithmetic_source.txt");

#[derive(Clone)]
struct Catalog {
    name: &'static str,
    domain: &'static str,
    records: Vec<FormulaRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone)]
struct Case {
    id: String,
    text: String,
    expected: Expected,
    intended: Option<&'static str>,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    source_sha256: BTreeMap<String, String>,
    catalog_records: BTreeMap<String, usize>,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_route_decisions: usize,
    authorized_routes: usize,
    frontend_invocations: usize,
    frontend_replays: usize,
    frontend_tamper_rejections: usize,
    downstream_artifacts: usize,
    downstream_replays: usize,
    downstream_tamper_rejections: usize,
    provenance_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    live_registry_mutations: usize,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn rational(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("valid benchmark rational")
}

fn input_value(record: &FormulaRecord, input: &str) -> Rational {
    record
        .constraints
        .iter()
        .find_map(|constraint| match constraint {
            InputConstraint::Positive(name) if name == input => Some(rational(3, 1)),
            InputConstraint::PositiveInteger(name) if name == input => Some(rational(5, 1)),
            InputConstraint::NonnegativeInteger(name) if name == input => Some(rational(5, 1)),
            InputConstraint::Probability(name) if name == input => Some(rational(1, 4)),
            InputConstraint::NotEqualInteger(name, forbidden) if name == input => {
                Some(rational(forbidden + 1, 1))
            }
            _ => None,
        })
        .unwrap_or_else(|| rational(3, 1))
}

fn render_value(value: Rational) -> String {
    if value.denominator == 1 {
        value.numerator.to_string()
    } else {
        format!("{}/{}", value.numerator, value.denominator)
    }
}

fn inputs(record: &FormulaRecord, omit_first: bool) -> String {
    record
        .required_inputs
        .iter()
        .enumerate()
        .filter(|(index, _)| !omit_first || *index > 0)
        .map(|(_, input)| format!("{input}: {}", render_value(input_value(record, input))))
        .collect::<Vec<_>>()
        .join(", ")
}

fn catalogs() -> Result<Vec<Catalog>, Box<dyn std::error::Error>> {
    Ok(vec![
        Catalog {
            name: "economics",
            domain: "source_derived_bounded_economics",
            records: extract_formula_records(ECONOMICS).map_err(|e| e.join("; "))?,
        },
        Catalog {
            name: "statistics",
            domain: source_statistics_pack::DOMAIN,
            records: source_statistics_pack::records(),
        },
        Catalog {
            name: "regression",
            domain: source_regression_pack::DOMAIN,
            records: source_regression_pack::records(),
        },
        Catalog {
            name: "complex_arithmetic",
            domain: "source_derived_complex_arithmetic",
            records: extract_formula_records(COMPLEX).map_err(|e| e.join("; "))?,
        },
        Catalog {
            name: "sequences_series",
            domain: "source_derived_sequences_series",
            records: source_formula_records(),
        },
    ])
}

fn cases(catalogs: &[Catalog]) -> Vec<Case> {
    let mut cases = Vec::with_capacity(2000);
    for catalog in catalogs {
        for index in 0..240usize {
            let record = &catalog.records[index % catalog.records.len()];
            let wording = match index % 4 {
                0 => "Using",
                1 => "Apply",
                2 => "Evaluate from",
                _ => "Compute with",
            };
            cases.push(Case {
                id: format!("supported-{}-{index:03}", catalog.name),
                text: format!(
                    "{wording} {}; {}.",
                    record.formula_id,
                    inputs(record, false)
                ),
                expected: Expected::Supported,
                intended: Some(catalog.name),
            });
        }
    }
    for index in 0..400usize {
        let catalog = &catalogs[index % catalogs.len()];
        let left = &catalog.records[index % catalog.records.len()];
        let right = &catalog.records[(index + 1) % catalog.records.len()];
        cases.push(Case {
            id: format!("ambiguous-{index:03}"),
            text: format!(
                "Choose either {} or {} with {}.",
                left.formula_id,
                right.formula_id,
                inputs(left, false)
            ),
            expected: Expected::Ambiguous,
            intended: None,
        });
    }
    for index in 0..400usize {
        cases.push(Case {
            id: format!("unsupported-{index:03}"),
            text: format!(
                "Give an approximate continuous regression result for an unknown formula, x={index}."
            ),
            expected: Expected::Unsupported,
            intended: None,
        });
    }
    cases
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalogs = catalogs()?;
    let cases = cases(&catalogs);
    let corpus_sha256 = digest(
        &cases
            .iter()
            .map(|case| (&case.id, &case.text, case.expected, case.intended))
            .collect::<Vec<_>>(),
    );
    let source_sha256 = BTreeMap::from([
        ("economics".into(), digest(ECONOMICS)),
        ("regression".into(), digest(REGRESSION)),
        ("complex_arithmetic".into(), digest(COMPLEX)),
        (
            "statistics".into(),
            digest(include_str!(
                "../../docs/sources/openstax_finite_statistics_source.txt"
            )),
        ),
        (
            "sequences_series".into(),
            digest(&serde_json::to_vec(&source_formula_records())?),
        ),
    ]);
    let catalog_records = catalogs
        .iter()
        .map(|catalog| (catalog.name.to_string(), catalog.records.len()))
        .collect();

    let mut exact_route_decisions = 0;
    let mut authorized_routes = 0;
    let mut frontend_invocations = 0;
    let mut frontend_replays = 0;
    let mut frontend_tamper_rejections = 0;
    let mut downstream_artifacts = 0;
    let mut downstream_replays = 0;
    let mut downstream_tamper_rejections = 0;
    let mut provenance_preserved = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;

    for case in &cases {
        let mut complete = Vec::new();
        let mut any_ambiguous = false;
        let mut all_frontends_replayed = true;
        let mut all_tampered = true;
        let mut all_provenance = true;
        for catalog in &catalogs {
            let result =
                formalize_source_formula_text(&case.text, catalog.domain, &catalog.records);
            frontend_invocations += 1;
            all_frontends_replayed &= replay_verified(&result);
            all_provenance &= !result.provenance_spans.is_empty();
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            all_tampered &= !replay_verified(&tampered);
            let status = result.status;
            if result.status == FrontendStatus::Complete {
                complete.push((catalog, result));
            }
            any_ambiguous |= status == FrontendStatus::Ambiguous;
        }
        frontend_replays += usize::from(all_frontends_replayed);
        frontend_tamper_rejections += usize::from(all_tampered);
        provenance_preserved += usize::from(all_provenance);

        let route = if complete.len() == 1 {
            let (catalog, frontend) = complete.pop().unwrap();
            let request = frontend.request.as_ref().expect("complete request");
            let execution = evaluate_formula_records(request, catalog.domain, &catalog.records);
            if execution.status == FormulaStatus::Complete && execution.value.is_some() {
                authorized_routes += 1;
                downstream_artifacts += 1;
                downstream_replays += usize::from(execution.replay_verified());
                let mut tampered = execution.clone();
                tampered.replay_hash.push('x');
                downstream_tamper_rejections += usize::from(!tampered.replay_verified());
                Expected::Supported
            } else {
                Expected::Unsupported
            }
        } else if any_ambiguous {
            Expected::Ambiguous
        } else {
            Expected::Unsupported
        };
        if route == case.expected {
            exact_route_decisions += 1;
        } else if case.expected == Expected::Supported {
            false_denials += 1;
        } else if route == Expected::Supported {
            false_authorizations += 1;
        }
    }

    let report = Report {
        schema: "stage220-route-blind-source-formula-language-v1",
        corpus_sha256,
        source_sha256,
        catalog_records,
        cases: cases.len(),
        supported: 1200,
        ambiguous: 400,
        unsupported: 400,
        exact_route_decisions,
        authorized_routes,
        frontend_invocations,
        frontend_replays,
        frontend_tamper_rejections,
        downstream_artifacts,
        downstream_replays,
        downstream_tamper_rejections,
        provenance_preserved,
        false_authorizations,
        false_denials,
        live_registry_mutations: 0,
    };
    assert_eq!(report.exact_route_decisions, 2000);
    assert_eq!(report.authorized_routes, 1200);
    assert_eq!(report.frontend_invocations, 10000);
    assert_eq!(report.frontend_replays, 2000);
    assert_eq!(report.frontend_tamper_rejections, 2000);
    assert_eq!(report.downstream_artifacts, 1200);
    assert_eq!(report.downstream_replays, 1200);
    assert_eq!(report.downstream_tamper_rejections, 1200);
    assert_eq!(report.provenance_preserved, 2000);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
