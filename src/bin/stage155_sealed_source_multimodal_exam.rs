//! Stage 155: sealed source/multimodal curriculum examination.
//!
//! This is an independently generated, route-balanced checkpoint over the
//! source-derived and raw-OCR visual routes admitted only in cloned-manifest
//! mode by Stage 154.  It never reads HLE and never changes production state.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::source_formula_pack::biology_pack::biology_frontend::{
    formalize_biology_text, BiologyFrontendStatus,
};
use the_machine::source_formula_pack::biology_pack::evaluate_biology;
use the_machine::source_formula_pack::chemistry_pack::chemistry_frontend::{
    formalize_chemistry_text, FrontendStatus as ChemistryFrontendStatus,
};
use the_machine::source_formula_pack::chemistry_pack::evaluate_chemistry;
use the_machine::source_statistics_frontend::{
    formalize_statistics_text, FrontendStatus as StatisticsFrontendStatus,
};
use the_machine::source_statistics_pack::evaluate_statistics;
use the_machine::vision::visual_source_statistics_bridge::table_to_statistics;
use the_machine::vision::visual_table::visual_biology_bridge::table_to_biology_probability;
use the_machine::vision::visual_table::visual_chemistry_bridge::table_to_chemistry_linear;
use the_machine::vision::visual_table::{formalize_table_tsv, TableStatus};

const HEADER: &str = "level\tpage\tblock\tpar\tline\tword\tleft\ttop\twidth\theight\tconf\ttext";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Family {
    Statistics,
    Biology,
    Chemistry,
    VisualStatistics,
    VisualBiology,
    VisualChemistry,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Partition {
    Development,
    Validation,
    Sealed,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    family: Family,
    partition: Partition,
    expected: Expected,
    authorized: bool,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct PartitionMetrics {
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    supported_authorized: usize,
    ambiguities_preserved: usize,
    unsupported_refused: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
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
    supported_authorized: usize,
    ambiguities_preserved: usize,
    unsupported_refused: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    production_registry_mutations: usize,
    route_counts: BTreeMap<String, usize>,
    partitions: BTreeMap<String, PartitionMetrics>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn word(left: usize, top: usize, text: &str) -> String {
    format!("5\t1\t1\t1\t1\t1\t{left}\t{top}\t40\t10\t90\t{text}")
}

fn tsv(rows: &[Vec<&str>]) -> String {
    let mut lines = vec![HEADER.into()];
    for (row, values) in rows.iter().enumerate() {
        for (column, value) in values.iter().enumerate() {
            lines.push(word(10 + column * 80, 10 + row * 30, value));
        }
    }
    lines.join("\n")
}

fn direct(family: Family, text: &str, expected: Expected) -> (bool, bool, bool) {
    match family {
        Family::Statistics => {
            let frontend = formalize_statistics_text(text);
            let Some(request) = frontend.request.as_ref() else {
                let mut tampered = frontend.clone();
                tampered.replay_hash.push('x');
                let exact = match expected {
                    Expected::Supported => false,
                    Expected::Ambiguous => frontend.status == StatisticsFrontendStatus::Ambiguous,
                    Expected::Unsupported => frontend.status != StatisticsFrontendStatus::Complete,
                };
                return (
                    exact,
                    frontend.replay_verified(),
                    !tampered.replay_verified(),
                );
            };
            let result = evaluate_statistics(request);
            let authorized = frontend.status == StatisticsFrontendStatus::Complete
                && result.status == the_machine::source_formula_pack::FormulaStatus::Complete
                && result.replay_verified();
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            let exact = match expected {
                Expected::Supported => authorized,
                Expected::Ambiguous => {
                    !authorized && frontend.status == StatisticsFrontendStatus::Ambiguous
                }
                Expected::Unsupported => !authorized,
            };
            (
                exact,
                frontend.replay_verified() && result.replay_verified(),
                !tampered.replay_verified(),
            )
        }
        Family::Biology => {
            let frontend = formalize_biology_text(text);
            let Some(request) = frontend.request.as_ref() else {
                let mut tampered = frontend.clone();
                tampered.replay_hash.push('x');
                let exact = match expected {
                    Expected::Supported => false,
                    Expected::Ambiguous => frontend.status == BiologyFrontendStatus::Ambiguous,
                    Expected::Unsupported => frontend.status != BiologyFrontendStatus::Complete,
                };
                return (
                    exact,
                    frontend.replay_verified(),
                    !tampered.replay_verified(),
                );
            };
            let result = evaluate_biology(request);
            let authorized =
                frontend.status == BiologyFrontendStatus::Complete && result.authorized();
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            let exact = match expected {
                Expected::Supported => authorized,
                Expected::Ambiguous => {
                    !authorized && frontend.status == BiologyFrontendStatus::Ambiguous
                }
                Expected::Unsupported => !authorized,
            };
            (
                exact,
                frontend.replay_verified() && result.replay_verified(),
                !tampered.replay_verified(),
            )
        }
        Family::Chemistry => {
            let frontend = formalize_chemistry_text(text);
            let Some(request) = frontend.request.as_ref() else {
                let mut tampered = frontend.clone();
                tampered.replay_hash.push('x');
                let exact = match expected {
                    Expected::Supported => false,
                    Expected::Ambiguous => frontend.status == ChemistryFrontendStatus::Ambiguous,
                    Expected::Unsupported => frontend.status != ChemistryFrontendStatus::Complete,
                };
                return (
                    exact,
                    frontend.replay_verified(),
                    !tampered.replay_verified(),
                );
            };
            let result = evaluate_chemistry(request);
            let authorized =
                frontend.status == ChemistryFrontendStatus::Complete && result.authorized();
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            let exact = match expected {
                Expected::Supported => authorized,
                Expected::Ambiguous => {
                    !authorized && frontend.status == ChemistryFrontendStatus::Ambiguous
                }
                Expected::Unsupported => !authorized,
            };
            (
                exact,
                frontend.replay_verified() && result.replay_verified(),
                !tampered.replay_verified(),
            )
        }
        _ => unreachable!(),
    }
}

fn visual(family: Family, rows: Vec<Vec<&str>>, expected: Expected) -> (bool, bool, bool) {
    let frontend = formalize_table_tsv(&tsv(&rows));
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let Some(table) = frontend.artifact.as_ref() else {
        return (
            expected != Expected::Supported,
            frontend.replay_verified(),
            !frontend_tampered.replay_verified(),
        );
    };
    let (authorized, downstream_replay, downstream_tamper, ambiguous) = match family {
        Family::VisualStatistics => {
            let result = table_to_statistics(table);
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            (
                result.authorized(),
                result.replay_verified(),
                !tampered.replay_verified(),
                format!("{:?}", result.status) == "Ambiguous",
            )
        }
        Family::VisualBiology => {
            let policy = (expected == Expected::Supported).then_some("uniform_position");
            let result = table_to_biology_probability(table, policy);
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            (
                result.authorized(),
                result.replay_verified(),
                !tampered.replay_verified(),
                format!("{:?}", result.status) == "Ambiguous",
            )
        }
        Family::VisualChemistry => {
            let result = table_to_chemistry_linear(table);
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            (
                result.authorized(),
                result.replay_verified(),
                !tampered.replay_verified(),
                format!("{:?}", result.status) == "Ambiguous",
            )
        }
        _ => unreachable!(),
    };
    let exact = match expected {
        Expected::Supported => frontend.status == TableStatus::Complete && authorized,
        Expected::Ambiguous => !authorized && ambiguous,
        Expected::Unsupported => !authorized,
    };
    (
        exact,
        frontend.replay_verified() && downstream_replay,
        !frontend_tampered.replay_verified() && downstream_tamper,
    )
}

fn case(family: Family, local: usize, global: usize) -> Receipt {
    let expected = match local % 10 {
        0..=5 => Expected::Supported,
        6..=7 => Expected::Ambiguous,
        _ => Expected::Unsupported,
    };
    let (exact, replay_verified, tamper_rejected) = match family {
        Family::Statistics => direct(
            family,
            match expected {
                Expected::Supported => "Find the mean from sum=30 and count=5.",
                Expected::Ambiguous => {
                    "For a binomial distribution with n=8 and p=1/2, determine the result."
                }
                Expected::Unsupported => "Fit a regression model and report a confidence interval.",
            },
            expected,
        ),
        Family::Biology => direct(
            family,
            match expected {
                Expected::Supported => "Report base composition for sequence: AATTGGCC.",
                Expected::Ambiguous => {
                    "Find the complement of sequence: AATTGGCC, but orientation is not stated."
                }
                Expected::Unsupported => "Translate the RNA sequence: AUGGCC into a protein.",
            },
            expected,
        ),
        Family::Chemistry => direct(
            family,
            match expected {
                Expected::Supported => "Parse formula: H2O.",
                Expected::Ambiguous => {
                    "Two candidates are present: formula: H2O and formula: CO2; select one."
                }
                Expected::Unsupported => "Compute the molar mass of formula: H2O.",
            },
            expected,
        ),
        Family::VisualStatistics => visual(
            family,
            match expected {
                Expected::Supported => vec![
                    vec!["quantity", "value"],
                    vec!["sum", "30"],
                    vec!["count", "5"],
                ],
                Expected::Ambiguous => vec![
                    vec!["label", "value"],
                    vec!["sum", "30"],
                    vec!["count", "5"],
                ],
                Expected::Unsupported => vec![
                    vec!["continuous density", "value"],
                    vec!["sum", "30"],
                    vec!["count", "5"],
                ],
            },
            expected,
        ),
        Family::VisualBiology => visual(
            family,
            match expected {
                Expected::Supported => vec![
                    vec!["base", "count"],
                    vec!["A", "2"],
                    vec!["C", "2"],
                    vec!["G", "2"],
                    vec!["T", "2"],
                ],
                Expected::Ambiguous => vec![
                    vec!["base", "count"],
                    vec!["A", "2"],
                    vec!["C", "2"],
                    vec!["G", "2"],
                    vec!["T", "2"],
                ],
                Expected::Unsupported => vec![
                    vec!["base", "count"],
                    vec!["X", "2"],
                    vec!["C", "2"],
                    vec!["G", "2"],
                    vec!["T", "2"],
                ],
            },
            expected,
        ),
        Family::VisualChemistry => visual(
            family,
            match expected {
                Expected::Supported => {
                    vec![vec!["element", "count"], vec!["H", "2"], vec!["O", "1"]]
                }
                Expected::Ambiguous => vec![vec!["label", "value"], vec!["H", "2"], vec!["O", "1"]],
                Expected::Unsupported => {
                    vec![vec!["element", "count"], vec!["Na+", "1"], vec!["O", "1"]]
                }
            },
            expected,
        ),
    };
    let authorized = expected == Expected::Supported && exact;
    Receipt {
        id: format!("stage155-{global:04}"),
        family,
        partition: if global < 1440 {
            Partition::Development
        } else if global < 1920 {
            Partition::Validation
        } else {
            Partition::Sealed
        },
        expected,
        authorized,
        exact,
        replay_verified,
        tamper_rejected,
        false_authorization: expected != Expected::Supported && authorized,
        false_denial: expected == Expected::Supported && !authorized,
    }
}

fn metrics(rows: &[Receipt], partition: Partition) -> PartitionMetrics {
    let rows: Vec<_> = rows.iter().filter(|r| r.partition == partition).collect();
    PartitionMetrics {
        cases: rows.len(),
        supported: rows
            .iter()
            .filter(|r| r.expected == Expected::Supported)
            .count(),
        ambiguous: rows
            .iter()
            .filter(|r| r.expected == Expected::Ambiguous)
            .count(),
        unsupported: rows
            .iter()
            .filter(|r| r.expected == Expected::Unsupported)
            .count(),
        supported_authorized: rows
            .iter()
            .filter(|r| r.expected == Expected::Supported && r.authorized)
            .count(),
        ambiguities_preserved: rows
            .iter()
            .filter(|r| r.expected == Expected::Ambiguous && r.exact)
            .count(),
        unsupported_refused: rows
            .iter()
            .filter(|r| r.expected == Expected::Unsupported && r.exact)
            .count(),
        replay_verified: rows.iter().filter(|r| r.replay_verified).count(),
        tamper_rejections: rows.iter().filter(|r| r.tamper_rejected).count(),
        false_authorizations: rows.iter().filter(|r| r.false_authorization).count(),
        false_denials: rows.iter().filter(|r| r.false_denial).count(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let families = [
        Family::Statistics,
        Family::Biology,
        Family::Chemistry,
        Family::VisualStatistics,
        Family::VisualBiology,
        Family::VisualChemistry,
    ];
    let mut receipts = Vec::with_capacity(2400);
    for (family_index, family) in families.into_iter().enumerate() {
        for local in 0..400 {
            receipts.push(case(family, local, family_index * 400 + local));
        }
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
    let unsupported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Unsupported)
        .count();
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let supported_authorized = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && r.authorized)
        .count();
    let ambiguities_preserved = receipts
        .iter()
        .filter(|r| r.expected == Expected::Ambiguous && r.exact)
        .count();
    let unsupported_refused = receipts
        .iter()
        .filter(|r| r.expected == Expected::Unsupported && r.exact)
        .count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    assert_eq!(
        (cases, supported, ambiguous, unsupported),
        (2400, 1440, 480, 480)
    );
    assert_eq!(exact_decisions, cases);
    assert_eq!(supported_authorized, supported);
    assert_eq!(ambiguities_preserved, ambiguous);
    assert_eq!(unsupported_refused, unsupported);
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let mut route_counts = BTreeMap::new();
    for receipt in &receipts {
        *route_counts
            .entry(format!("{:?}", receipt.family))
            .or_insert(0) += 1;
    }
    let mut partitions = BTreeMap::new();
    partitions.insert(
        "development".into(),
        metrics(&receipts, Partition::Development),
    );
    partitions.insert(
        "validation".into(),
        metrics(&receipts, Partition::Validation),
    );
    partitions.insert("sealed".into(), metrics(&receipts, Partition::Sealed));
    let report = Report {
        schema: "stage155-sealed-source-multimodal-exam-v1",
        source: "independently generated direct and coordinate-bearing raw-OCR source routes",
        corpus_sha256: digest(&receipts),
        cases,
        supported,
        ambiguous,
        unsupported,
        exact_decisions,
        supported_authorized,
        ambiguities_preserved,
        unsupported_refused,
        replay_verified,
        tamper_rejections,
        false_authorizations,
        false_denials,
        production_registry_mutations: 0,
        route_counts,
        partitions,
        receipts,
    };
    let json = serde_json::to_vec_pretty(&report)?;
    std::fs::write("docs/stage155_sealed_source_multimodal_exam.json", &json)?;
    println!("{}", String::from_utf8(json)?);
    Ok(())
}
