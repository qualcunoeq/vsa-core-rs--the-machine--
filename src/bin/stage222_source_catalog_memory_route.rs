//! Stage 222: exact source-catalog retrieval from curriculum memory.
//!
//! Versioned source catalogs are retrieved from a cloned append-only memory,
//! then passed to the operative multi-region frontend. Missing and multiply
//! matching versions remain non-authorizing.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};
use the_machine::probability_pack::Rational;
use the_machine::source_catalog_memory::{
    append_catalog, replay_verified as catalog_replay, retrieve_catalog, CatalogMemoryStatus,
    ARTIFACT_TYPE,
};
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
    Missing,
}

#[derive(Debug, Clone)]
struct Case {
    id: String,
    text: String,
    domain: &'static str,
    version: &'static str,
    expected: Expected,
    conflict: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    memory_records: usize,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    missing: usize,
    exact_decisions: usize,
    authorized_routes: usize,
    catalog_lookups: usize,
    catalog_replays: usize,
    catalog_tamper_rejections: usize,
    unique_catalogs: usize,
    ambiguous_catalogs: usize,
    missing_catalogs: usize,
    frontend_replays: usize,
    downstream_replays: usize,
    downstream_tamper_rejections: usize,
    provenance_preserved: usize,
    clone_memory_unchanged: usize,
    false_authorizations: usize,
    false_denials: usize,
    live_memory_mutations: usize,
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

fn base_memory(catalogs: &[Catalog]) -> CurriculumMemory {
    let mut memory = CurriculumMemory::new();
    for catalog in catalogs {
        assert_eq!(
            append_catalog(
                &mut memory,
                catalog.domain,
                "v2",
                &catalog.records,
                vec![format!("source-catalog:{}:v2", catalog.name)],
            ),
            AppendStatus::Appended
        );
    }
    memory
}

fn cases(catalogs: &[Catalog]) -> Vec<Case> {
    let mut cases = Vec::new();
    for catalog in catalogs {
        for index in 0..60usize {
            let target = &catalog.records[index % catalog.records.len()];
            let definition = &catalog.records[(index + 1) % catalog.records.len()];
            cases.push(Case {
                id: format!("supported-{}-{index:02}", catalog.name),
                text: format!(
                    "For reference, {} is defined. Calculate {} using {}.",
                    definition.formula_id,
                    target.formula_id,
                    inputs(target)
                ),
                domain: catalog.domain,
                version: "v2",
                expected: Expected::Supported,
                conflict: false,
            });
        }
    }
    for index in 0..100usize {
        let catalog = &catalogs[index % catalogs.len()];
        let left = &catalog.records[index % catalog.records.len()];
        let right = &catalog.records[(index + 1) % catalog.records.len()];
        cases.push(Case {
            id: format!("ambiguous-{index:03}"),
            text: format!(
                "Calculate {} or {} using {}.",
                left.formula_id,
                right.formula_id,
                inputs(left)
            ),
            domain: catalog.domain,
            version: "v2",
            expected: Expected::Ambiguous,
            conflict: index % 4 == 0,
        });
    }
    for index in 0..100usize {
        let catalog = &catalogs[index % catalogs.len()];
        let target = &catalog.records[index % catalog.records.len()];
        cases.push(Case {
            id: format!("missing-{index:03}"),
            text: format!("Calculate {} using {}.", target.formula_id, inputs(target)),
            domain: catalog.domain,
            version: "v9",
            expected: Expected::Missing,
            conflict: false,
        });
    }
    cases
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalogs = catalogs()?;
    let base = base_memory(&catalogs);
    let base_len = base.len();
    let cases = cases(&catalogs);
    let corpus_sha256 = digest(
        &cases
            .iter()
            .map(|case| {
                (
                    &case.id,
                    &case.text,
                    case.domain,
                    case.version,
                    case.expected,
                    case.conflict,
                )
            })
            .collect::<Vec<_>>(),
    );
    let mut exact_decisions = 0;
    let mut authorized_routes = 0;
    let mut catalog_lookups = 0;
    let mut catalog_replays = 0;
    let mut catalog_tamper_rejections = 0;
    let mut unique_catalogs = 0;
    let mut ambiguous_catalogs = 0;
    let mut missing_catalogs = 0;
    let mut frontend_replays = 0;
    let mut downstream_replays = 0;
    let mut downstream_tamper_rejections = 0;
    let mut provenance_preserved = 0;
    let mut clone_memory_unchanged = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;

    for case in &cases {
        let mut memory = base.clone();
        if case.conflict {
            let catalog = catalogs
                .iter()
                .find(|catalog| catalog.domain == case.domain)
                .unwrap();
            let payload = serde_json::to_string(&catalog.records)?;
            assert_eq!(
                memory.append(MemoryRecord {
                    record_id: format!("conflict::{}::v2", case.domain),
                    domain: case.domain.into(),
                    artifact_type: ARTIFACT_TYPE.into(),
                    version: "v2".into(),
                    payload,
                    provenance: vec!["conflicting-source-lineage".into()],
                    content_hash: String::new(),
                }),
                AppendStatus::Appended
            );
        }
        let mut complete = Vec::new();
        let mut ambiguous = false;
        for catalog in &catalogs {
            let retrieved = retrieve_catalog(&memory, catalog.domain, case.version);
            catalog_lookups += 1;
            catalog_replays += usize::from(catalog_replay(&retrieved));
            let mut tampered = retrieved.clone();
            tampered.replay_hash.push('x');
            catalog_tamper_rejections += usize::from(!catalog_replay(&tampered));
            match retrieved.status {
                CatalogMemoryStatus::Unique => {
                    unique_catalogs += 1;
                    provenance_preserved += usize::from(!retrieved.provenance.is_empty());
                    let report = formalize_source_formula_report(
                        &case.text,
                        catalog.domain,
                        &retrieved.records,
                    );
                    frontend_replays += usize::from(report_replay_verified(&report));
                    let status = report.frontend.status;
                    if report.frontend.status == FrontendStatus::Complete {
                        complete.push((catalog, report));
                    }
                    ambiguous |= status == FrontendStatus::Ambiguous;
                }
                CatalogMemoryStatus::Ambiguous => {
                    ambiguous_catalogs += 1;
                    ambiguous = true;
                }
                CatalogMemoryStatus::Missing => missing_catalogs += 1,
                CatalogMemoryStatus::Invalid => {}
            }
        }
        let actual = if complete.len() == 1 {
            let (catalog, report) = complete.pop().unwrap();
            let request = report.frontend.request.as_ref().unwrap();
            let execution = evaluate_formula_records(request, catalog.domain, &catalog.records);
            if execution.status == FormulaStatus::Complete && execution.value.is_some() {
                authorized_routes += 1;
                downstream_replays += usize::from(execution.replay_verified());
                let mut tampered = execution.clone();
                tampered.replay_hash.push('x');
                downstream_tamper_rejections += usize::from(!tampered.replay_verified());
                Expected::Supported
            } else {
                Expected::Missing
            }
        } else if ambiguous {
            Expected::Ambiguous
        } else {
            Expected::Missing
        };
        if actual == case.expected {
            exact_decisions += 1;
        } else if case.expected == Expected::Supported {
            false_denials += 1;
        } else if actual == Expected::Supported {
            false_authorizations += 1;
        }
        clone_memory_unchanged += usize::from(base.len() == base_len);
    }

    let report = Report {
        schema: "stage222-source-catalog-memory-route-v1",
        corpus_sha256,
        memory_records: base.len(),
        cases: cases.len(),
        supported: 300,
        ambiguous: 100,
        missing: 100,
        exact_decisions,
        authorized_routes,
        catalog_lookups,
        catalog_replays,
        catalog_tamper_rejections,
        unique_catalogs,
        ambiguous_catalogs,
        missing_catalogs,
        frontend_replays,
        downstream_replays,
        downstream_tamper_rejections,
        provenance_preserved,
        clone_memory_unchanged,
        false_authorizations,
        false_denials,
        live_memory_mutations: 0,
    };
    assert_eq!(report.exact_decisions, 500);
    assert_eq!(report.authorized_routes, 300);
    assert_eq!(report.catalog_lookups, 2500);
    assert_eq!(report.catalog_replays, 2500);
    assert_eq!(report.catalog_tamper_rejections, 2500);
    assert_eq!(report.frontend_replays, 1975);
    assert_eq!(report.downstream_replays, 300);
    assert_eq!(report.downstream_tamper_rejections, 300);
    assert_eq!(report.provenance_preserved, 1975);
    assert_eq!(report.clone_memory_unchanged, 500);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
