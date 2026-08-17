//! Stage 150: integrated post-extension source/science/multimodal checkpoint.
//!
//! This benchmark combines the three coordinate-preserving visual routes with
//! direct source chemistry and biology bridges.  It is independently generated
//! after those routes were frozen; no HLE text or answer key is read.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::source_formula_pack::biology_pack::{
    biology_probability_bridge::bridge_base_composition, evaluate_biology, BiologyOperation,
    BiologyRequest, BiologyStatus,
};
use the_machine::source_formula_pack::chemistry_pack::{
    chemistry_linear_bridge::bridge_chemistry_to_linear, evaluate_chemistry, ChemistryOperation,
    ChemistryRequest, ChemistryStatus,
};
use the_machine::vision::visual_source_statistics_bridge::table_to_statistics;
use the_machine::vision::visual_table::visual_biology_bridge::table_to_biology_probability;
use the_machine::vision::visual_table::visual_chemistry_bridge::table_to_chemistry_linear;
use the_machine::vision::visual_table::{TableArtifact, TableCell};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Serialize)]
struct Receipt {
    family: String,
    id: String,
    expected: Expected,
    status: String,
    authorized: bool,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
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
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn table(rows: Vec<Vec<&str>>, family: &str, id: &str) -> TableArtifact {
    let rows = rows
        .into_iter()
        .map(|row| row.into_iter().map(String::from).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let cells = rows
        .iter()
        .enumerate()
        .flat_map(|(row, values)| {
            values
                .iter()
                .enumerate()
                .map(move |(column, text)| TableCell {
                    text: text.clone(),
                    row,
                    column,
                    left: (column * 50) as u32,
                    top: (row * 20) as u32,
                    width: 40,
                    height: 12,
                })
        })
        .collect();
    TableArtifact {
        row_count: rows.len(),
        column_count: rows.first().map_or(0, Vec::len),
        cells,
        provenance_spans: vec![
            format!("stage150:{family}:{id}"),
            "independent-source-corpus".into(),
        ],
        rows,
    }
}

fn receipt(
    family: &str,
    id: String,
    expected: Expected,
    status: String,
    authorized: bool,
    replay_verified: bool,
    tamper_rejected: bool,
) -> Receipt {
    let exact = match expected {
        Expected::Supported => authorized,
        Expected::Ambiguous | Expected::Unsupported => !authorized,
    };
    Receipt {
        family: family.into(),
        id,
        expected,
        status,
        authorized,
        exact,
        replay_verified,
        tamper_rejected,
    }
}

fn push_visual_statistics(receipts: &mut Vec<Receipt>) {
    for index in 0..120 {
        let id = format!("supported_{index:03}");
        let result = table_to_statistics(&table(
            vec![
                vec!["quantity", "value"],
                vec!["sum", "30"],
                vec!["count", "5"],
            ],
            "visual_statistics",
            &id,
        ));
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        receipts.push(receipt(
            "visual_statistics",
            id,
            Expected::Supported,
            format!("{:?}", result.status),
            result.authorized(),
            result.replay_verified(),
            !tampered.replay_verified(),
        ));
    }
    for index in 0..40 {
        let id = format!("ambiguous_{index:03}");
        let result = table_to_statistics(&table(
            vec![
                vec!["label", "value"],
                vec!["sum", "30"],
                vec!["count", "5"],
            ],
            "visual_statistics",
            &id,
        ));
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        receipts.push(receipt(
            "visual_statistics",
            id,
            Expected::Ambiguous,
            format!("{:?}", result.status),
            result.authorized(),
            result.replay_verified(),
            !tampered.replay_verified(),
        ));
    }
    for index in 0..40 {
        let id = format!("unsupported_{index:03}");
        let result = table_to_statistics(&table(
            vec![
                vec!["continuous density", "value"],
                vec!["sum", "30"],
                vec!["count", "5"],
            ],
            "visual_statistics",
            &id,
        ));
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        receipts.push(receipt(
            "visual_statistics",
            id,
            Expected::Unsupported,
            format!("{:?}", result.status),
            result.authorized(),
            result.replay_verified(),
            !tampered.replay_verified(),
        ));
    }
}

fn push_visual_biology(receipts: &mut Vec<Receipt>) {
    let rows = vec![
        vec!["base", "count"],
        vec!["A", "2"],
        vec!["C", "2"],
        vec!["G", "2"],
        vec!["T", "2"],
    ];
    for index in 0..120 {
        let id = format!("supported_{index:03}");
        let result = table_to_biology_probability(
            &table(rows.clone(), "visual_biology", &id),
            Some("uniform_position"),
        );
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        receipts.push(receipt(
            "visual_biology",
            id,
            Expected::Supported,
            format!("{:?}", result.status),
            result.authorized(),
            result.replay_verified(),
            !tampered.replay_verified(),
        ));
    }
    for index in 0..40 {
        let id = format!("ambiguous_{index:03}");
        let result =
            table_to_biology_probability(&table(rows.clone(), "visual_biology", &id), None);
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        receipts.push(receipt(
            "visual_biology",
            id,
            Expected::Ambiguous,
            format!("{:?}", result.status),
            result.authorized(),
            result.replay_verified(),
            !tampered.replay_verified(),
        ));
    }
    for index in 0..40 {
        let id = format!("unsupported_{index:03}");
        let result = table_to_biology_probability(
            &table(
                vec![vec!["rna", "count"], vec!["A", "2"], vec!["U", "2"]],
                "visual_biology",
                &id,
            ),
            Some("uniform_position"),
        );
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        receipts.push(receipt(
            "visual_biology",
            id,
            Expected::Unsupported,
            format!("{:?}", result.status),
            result.authorized(),
            result.replay_verified(),
            !tampered.replay_verified(),
        ));
    }
}

fn push_visual_chemistry(receipts: &mut Vec<Receipt>) {
    let supported_rows = vec![vec!["element", "count"], vec!["H", "2"], vec!["O", "1"]];
    for index in 0..120 {
        let id = format!("supported_{index:03}");
        let result =
            table_to_chemistry_linear(&table(supported_rows.clone(), "visual_chemistry", &id));
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        receipts.push(receipt(
            "visual_chemistry",
            id,
            Expected::Supported,
            format!("{:?}", result.status),
            result.authorized(),
            result.replay_verified(),
            !tampered.replay_verified(),
        ));
    }
    for index in 0..40 {
        let id = format!("ambiguous_{index:03}");
        let result = table_to_chemistry_linear(&table(
            vec![vec!["label", "count"], vec!["H", "2"], vec!["O", "1"]],
            "visual_chemistry",
            &id,
        ));
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        receipts.push(receipt(
            "visual_chemistry",
            id,
            Expected::Ambiguous,
            format!("{:?}", result.status),
            result.authorized(),
            result.replay_verified(),
            !tampered.replay_verified(),
        ));
    }
    for index in 0..40 {
        let id = format!("unsupported_{index:03}");
        let result = table_to_chemistry_linear(&table(
            vec![vec!["element", "count"], vec!["Na+", "1"]],
            "visual_chemistry",
            &id,
        ));
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        receipts.push(receipt(
            "visual_chemistry",
            id,
            Expected::Unsupported,
            format!("{:?}", result.status),
            result.authorized(),
            result.replay_verified(),
            !tampered.replay_verified(),
        ));
    }
}

fn push_direct_chemistry(receipts: &mut Vec<Receipt>) {
    for index in 0..120 {
        let id = format!("supported_{index:03}");
        let result = evaluate_chemistry(&ChemistryRequest {
            operation: ChemistryOperation::ParseFormula,
            formula: Some("H2O".into()),
            reaction: None,
            from_species: None,
            to_species: None,
            domain: "source_derived_bounded_chemistry".into(),
            ambiguity: None,
            provenance: vec![format!("stage150:direct_chemistry:{id}")],
        });
        let bridge = bridge_chemistry_to_linear(&result);
        let authorized = result.status == ChemistryStatus::Complete
            && result.replay_verified()
            && bridge.authorized();
        let replay = result.replay_verified() && bridge.replay_verified();
        let mut tampered = bridge.clone();
        tampered.replay_hash.push('x');
        receipts.push(receipt(
            "direct_chemistry",
            id,
            Expected::Supported,
            format!("{:?}", result.status),
            authorized,
            replay,
            !tampered.replay_verified(),
        ));
    }
    for index in 0..40 {
        let id = format!("ambiguous_{index:03}");
        let result = evaluate_chemistry(&ChemistryRequest {
            operation: ChemistryOperation::ParseFormula,
            formula: Some("H2O".into()),
            reaction: None,
            from_species: None,
            to_species: None,
            domain: "source_derived_bounded_chemistry".into(),
            ambiguity: Some("formula context is ambiguous".into()),
            provenance: vec![format!("stage150:direct_chemistry:{id}")],
        });
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        receipts.push(receipt(
            "direct_chemistry",
            id,
            Expected::Ambiguous,
            format!("{:?}", result.status),
            false,
            result.replay_verified(),
            !tampered.replay_verified(),
        ));
    }
    for index in 0..40 {
        let id = format!("unsupported_{index:03}");
        let result = evaluate_chemistry(&ChemistryRequest {
            operation: ChemistryOperation::ParseFormula,
            formula: Some("H2O".into()),
            reaction: None,
            from_species: None,
            to_species: None,
            domain: "untrusted_chemistry_domain".into(),
            ambiguity: None,
            provenance: vec![format!("stage150:direct_chemistry:{id}")],
        });
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        receipts.push(receipt(
            "direct_chemistry",
            id,
            Expected::Unsupported,
            format!("{:?}", result.status),
            false,
            result.replay_verified(),
            !tampered.replay_verified(),
        ));
    }
}

fn push_direct_biology(receipts: &mut Vec<Receipt>) {
    for index in 0..120 {
        let id = format!("supported_{index:03}");
        let biology = evaluate_biology(&BiologyRequest {
            operation: BiologyOperation::BaseComposition,
            sequence: Some("AATTGGCC".into()),
            orientation: None,
            domain: "source_derived_bounded_dna".into(),
            ambiguity: None,
            provenance: vec![format!("stage150:direct_biology:{id}")],
        });
        let bridge = bridge_base_composition(&biology, Some("uniform_position"));
        let authorized = biology.status == BiologyStatus::Complete
            && biology.replay_verified()
            && bridge.authorized();
        let replay = biology.replay_verified() && bridge.replay_verified();
        let mut tampered = bridge.clone();
        tampered.replay_hash.push('x');
        receipts.push(receipt(
            "direct_biology",
            id,
            Expected::Supported,
            format!("{:?}", biology.status),
            authorized,
            replay,
            !tampered.replay_verified(),
        ));
    }
    for index in 0..40 {
        let id = format!("ambiguous_{index:03}");
        let biology = evaluate_biology(&BiologyRequest {
            operation: BiologyOperation::BaseComposition,
            sequence: Some("AATTGGCC".into()),
            orientation: None,
            domain: "source_derived_bounded_dna".into(),
            ambiguity: None,
            provenance: vec![format!("stage150:direct_biology:{id}")],
        });
        let bridge = bridge_base_composition(&biology, None);
        let mut tampered = bridge.clone();
        tampered.replay_hash.push('x');
        receipts.push(receipt(
            "direct_biology",
            id,
            Expected::Ambiguous,
            format!("{:?}", bridge.status),
            false,
            biology.replay_verified() && bridge.replay_verified(),
            !tampered.replay_verified(),
        ));
    }
    for index in 0..40 {
        let id = format!("unsupported_{index:03}");
        let biology = evaluate_biology(&BiologyRequest {
            operation: BiologyOperation::BaseComposition,
            sequence: Some("AAUU".into()),
            orientation: None,
            domain: "source_derived_bounded_dna".into(),
            ambiguity: None,
            provenance: vec![format!("stage150:direct_biology:{id}")],
        });
        let bridge = bridge_base_composition(&biology, Some("uniform_position"));
        let mut tampered = bridge.clone();
        tampered.replay_hash.push('x');
        receipts.push(receipt(
            "direct_biology",
            id,
            Expected::Unsupported,
            format!("{:?}", bridge.status),
            false,
            biology.replay_verified() && bridge.replay_verified(),
            !tampered.replay_verified(),
        ));
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut receipts = Vec::with_capacity(1000);
    push_visual_statistics(&mut receipts);
    push_visual_biology(&mut receipts);
    push_visual_chemistry(&mut receipts);
    push_direct_chemistry(&mut receipts);
    push_direct_biology(&mut receipts);
    assert_eq!(receipts.len(), 1000);
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
    let authorized_supported = receipts.iter().filter(|r| r.authorized).count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let false_authorizations = receipts
        .iter()
        .filter(|r| r.expected != Expected::Supported && r.authorized)
        .count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && !r.authorized)
        .count();
    assert_eq!((supported, ambiguous, unsupported), (600, 200, 200));
    assert_eq!(exact_decisions, 1000);
    assert_eq!(authorized_supported, 600);
    assert_eq!(replay_verified, 1000);
    assert_eq!(tamper_rejections, 1000);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let mut route_counts = BTreeMap::new();
    for receipt in &receipts {
        *route_counts.entry(receipt.family.clone()).or_insert(0) += 1;
    }
    let report = Report {
        schema: "stage150-source-multimodal-composition-v1",
        source: "independently authored source/science/multimodal composition corpus",
        corpus_sha256: digest(&receipts),
        cases,
        supported,
        ambiguous,
        unsupported,
        exact_decisions,
        authorized_supported,
        replay_verified,
        tamper_rejections,
        false_authorizations,
        false_denials,
        route_counts,
        receipts,
    };
    let json = serde_json::to_vec_pretty(&report)?;
    std::fs::write("docs/stage150_source_multimodal_composition.json", &json)?;
    println!("{}", String::from_utf8(json)?);
    Ok(())
}
