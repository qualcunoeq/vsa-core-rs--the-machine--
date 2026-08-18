//! Stage 221: multi-region target grounding for source formula reports.
//!
//! This is a follow-up to the route-blind catalog gate.  Reports contain a
//! definition or incidental formula before the requested operation.  The
//! generic frontend must select the operative target, preserve excluded
//! regions, and refuse multiple operative targets.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::probability_pack::Rational;
use the_machine::source_formula_frontend::{
    formalize_source_formula_report, report_replay_verified, FrontendStatus,
};
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, source_formula_records, FormulaRecord,
    FormulaStatus, InputConstraint,
};
use the_machine::{source_regression_pack, source_statistics_pack};

const ECONOMICS: &str = include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
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
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_route_decisions: usize,
    authorized_routes: usize,
    report_invocations: usize,
    report_replays: usize,
    report_tamper_rejections: usize,
    target_regions_preserved: usize,
    excluded_regions_preserved: usize,
    downstream_replays: usize,
    downstream_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    live_registry_mutations: usize,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn rational(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("valid rational")
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

fn render(value: Rational) -> String {
    if value.denominator == 1 {
        value.numerator.to_string()
    } else {
        format!("{}/{}", value.numerator, value.denominator)
    }
}

fn inputs(record: &FormulaRecord) -> String {
    record
        .required_inputs
        .iter()
        .map(|input| format!("{input}={}", render(input_value(record, input))))
        .collect::<Vec<_>>()
        .join(" and ")
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
            let target = &catalog.records[index % catalog.records.len()];
            let definition = &catalog.records[(index + 1) % catalog.records.len()];
            cases.push(Case {
                id: format!("supported-{}-{index:03}", catalog.name),
                text: format!(
                    "For reference, {} is defined in the source. Calculate {} using {}.",
                    definition.formula_id,
                    target.formula_id,
                    inputs(target)
                ),
                expected: Expected::Supported,
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
                "For reference, {} is defined. Calculate {} or {} using {}.",
                left.formula_id,
                left.formula_id,
                right.formula_id,
                inputs(left)
            ),
            expected: Expected::Ambiguous,
        });
    }
    for index in 0..400usize {
        cases.push(Case {
            id: format!("unsupported-{index:03}"),
            text: format!(
                "A reference formula is given. Calculate an approximate continuous result for x={index}."
            ),
            expected: Expected::Unsupported,
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
            .map(|case| (&case.id, &case.text, case.expected))
            .collect::<Vec<_>>(),
    );
    let mut exact_route_decisions = 0;
    let mut authorized_routes = 0;
    let mut report_invocations = 0;
    let mut report_replays = 0;
    let mut report_tamper_rejections = 0;
    let mut target_regions_preserved = 0;
    let mut excluded_regions_preserved = 0;
    let mut downstream_replays = 0;
    let mut downstream_tamper_rejections = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;

    for case in &cases {
        let mut complete = Vec::new();
        let mut any_ambiguous = false;
        let mut all_replayed = true;
        let mut all_tampered = true;
        let mut has_target = false;
        let mut has_excluded = false;
        for catalog in &catalogs {
            let result = formalize_source_formula_report(&case.text, catalog.domain, &catalog.records);
            report_invocations += 1;
            all_replayed &= report_replay_verified(&result);
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            all_tampered &= !report_replay_verified(&tampered);
            let status = result.frontend.status;
            has_target |= result.regions.iter().any(|region| {
                matches!(region.role, the_machine::source_formula_frontend::FormulaRegionRole::Target)
            });
            has_excluded |= result.regions.iter().any(|region| {
                matches!(region.role, the_machine::source_formula_frontend::FormulaRegionRole::Definition)
            });
            if result.frontend.status == FrontendStatus::Complete {
                complete.push((catalog, result));
            }
            any_ambiguous |= status == FrontendStatus::Ambiguous;
        }
        report_replays += usize::from(all_replayed);
        report_tamper_rejections += usize::from(all_tampered);
        target_regions_preserved += usize::from(has_target);
        excluded_regions_preserved += usize::from(has_excluded);

        let actual = if complete.len() == 1 {
            let (catalog, result) = complete.pop().unwrap();
            let request = result.frontend.request.as_ref().expect("complete request");
            let execution = evaluate_formula_records(request, catalog.domain, &catalog.records);
            if execution.status == FormulaStatus::Complete && execution.value.is_some() {
                authorized_routes += 1;
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
        if actual == case.expected {
            exact_route_decisions += 1;
        } else if case.expected == Expected::Supported {
            false_denials += 1;
        } else if actual == Expected::Supported {
            false_authorizations += 1;
        }
    }

    let report = Report {
        schema: "stage221-operative-formula-report-language-v1",
        corpus_sha256,
        cases: cases.len(),
        supported: 1200,
        ambiguous: 400,
        unsupported: 400,
        exact_route_decisions,
        authorized_routes,
        report_invocations,
        report_replays,
        report_tamper_rejections,
        target_regions_preserved,
        excluded_regions_preserved,
        downstream_replays,
        downstream_tamper_rejections,
        false_authorizations,
        false_denials,
        live_registry_mutations: 0,
    };
    assert_eq!(report.exact_route_decisions, 2000);
    assert_eq!(report.authorized_routes, 1200);
    assert_eq!(report.report_invocations, 10000);
    assert_eq!(report.report_replays, 2000);
    assert_eq!(report.report_tamper_rejections, 2000);
    assert_eq!(report.target_regions_preserved, 1200);
    assert_eq!(report.excluded_regions_preserved, 1600);
    assert_eq!(report.downstream_replays, 1200);
    assert_eq!(report.downstream_tamper_rejections, 1200);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
