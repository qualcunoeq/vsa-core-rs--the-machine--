//! Stage AH: source-derived scalar composition with finite probability.
//!
//! A source formula is evaluated independently for each explicit outcome and
//! then passed through a generic source-to-expectation bridge. The bridge
//! refuses rational values that cannot be represented by the finite integer
//! outcome artifact, malformed probability vectors, missing source results,
//! and ambiguous mappings.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::probability_pack::{ProbabilityArtifact, Rational};
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaResult,
};
use the_machine::source_probability_bridge::{
    bridge_source_scalars_to_expectation, SourceProbabilityBridgeStatus,
};

const DOMAIN: &str = "source_derived_bounded_economics";
const SOURCE_DOCUMENT: &str =
    include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const REPORT_JSON: &str = "docs/stage_ah_source_probability_composition.json";
const REPORT_MD: &str = "docs/stage_ah_source_probability_composition.md";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum Expected {
    Complete,
    Ambiguous,
    Refused,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    actual: SourceProbabilityBridgeStatus,
    exact: bool,
    value_correct: bool,
    source_replay: bool,
    bridge_replay: bool,
    bridge_tamper_rejected: bool,
    false_authorization: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_sha256: String,
    source_record_count: usize,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    complete_expectations: usize,
    source_replays: usize,
    bridge_replays: usize,
    bridge_tamper_rejections: usize,
    value_correct: usize,
    false_authorizations: usize,
    false_denials: usize,
    live_mutations: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn source_result(
    records: &[the_machine::source_formula_pack::FormulaRecord],
    price: i128,
    quantity: i128,
    id: &str,
) -> FormulaResult {
    evaluate_formula_records(
        &the_machine::source_formula_pack::FormulaRequest {
            formula: "total_revenue".into(),
            inputs: BTreeMap::from([
                ("price".into(), q(price, 1)),
                ("quantity".into(), q(quantity, 1)),
            ]),
            domain: DOMAIN.into(),
            ambiguity: None,
            provenance: vec![format!("stage-ah:{id}:source")],
        },
        DOMAIN,
        records,
    )
}

fn receipt(
    id: String,
    expected: Expected,
    bridge: the_machine::source_probability_bridge::SourceProbabilityBridgeResult,
    source_replay: bool,
    expected_value: Option<Rational>,
) -> Receipt {
    let actual = bridge.status;
    let exact = match expected {
        Expected::Complete => actual == SourceProbabilityBridgeStatus::Complete,
        Expected::Ambiguous => actual == SourceProbabilityBridgeStatus::Ambiguous,
        Expected::Refused => actual != SourceProbabilityBridgeStatus::Complete,
    };
    let value_correct = expected != Expected::Complete
        || matches!(
            bridge.expectation.as_ref().and_then(|result| result.artifact.as_ref()),
            Some(ProbabilityArtifact::Scalar(value)) if Some(value.clone()) == expected_value
        );
    let mut tampered = bridge.clone();
    tampered.replay_hash.push('x');
    Receipt {
        id,
        expected,
        actual,
        exact,
        value_correct,
        source_replay,
        bridge_replay: bridge.replay_verified(),
        bridge_tamper_rejected: !tampered.replay_verified(),
        false_authorization: expected != Expected::Complete
            && actual == SourceProbabilityBridgeStatus::Complete,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let records = extract_formula_records(SOURCE_DOCUMENT)
        .map_err(|errors| format!("source extraction failed: {errors:?}"))?;
    assert_eq!(records.len(), 5);
    let mut receipts = Vec::with_capacity(240);
    for index in 0..120 {
        let price = (index as i128 % 9) + 3;
        let source_results = [1, 2, 3]
            .into_iter()
            .map(|quantity| source_result(&records, price, quantity, &format!("supported-{index}")))
            .collect::<Vec<_>>();
        let bridge = bridge_source_scalars_to_expectation(
            vec!["q1".into(), "q2".into(), "q3".into()],
            vec![q(1, 4), q(1, 2), q(1, 4)],
            &source_results,
            None,
            vec![format!("stage-ah-supported-{index}:mapping")],
        );
        receipts.push(receipt(
            format!("supported-{index:03}"),
            Expected::Complete,
            bridge,
            source_results.iter().all(FormulaResult::replay_verified),
            Some(q(price * 2, 1)),
        ));
    }
    for index in 0..40 {
        let source_results = [1, 2, 3]
            .into_iter()
            .map(|quantity| source_result(&records, 5, quantity, &format!("ambiguous-{index}")))
            .collect::<Vec<_>>();
        let bridge = bridge_source_scalars_to_expectation(
            vec!["q1".into(), "q2".into(), "q3".into()],
            vec![q(1, 4), q(1, 2), q(1, 4)],
            &source_results,
            Some("the source-to-outcome mapping is ambiguous".into()),
            vec![format!("stage-ah-ambiguous-{index}:mapping")],
        );
        receipts.push(receipt(
            format!("ambiguous-{index:03}"),
            Expected::Ambiguous,
            bridge,
            source_results.iter().all(FormulaResult::replay_verified),
            None,
        ));
    }
    for index in 0..20 {
        let source_results = [1, 2, 3]
            .into_iter()
            .map(|quantity| {
                evaluate_formula_records(
                    &the_machine::source_formula_pack::FormulaRequest {
                        formula: "average_fixed_cost".into(),
                        inputs: BTreeMap::from([
                            ("fixed_cost".into(), q(5, 1)),
                            ("quantity".into(), q(quantity + 1, 1)),
                        ]),
                        domain: DOMAIN.into(),
                        ambiguity: None,
                        provenance: vec![format!("stage-ah-rational-{index}")],
                    },
                    DOMAIN,
                    &records,
                )
            })
            .collect::<Vec<_>>();
        let bridge = bridge_source_scalars_to_expectation(
            vec!["q1".into(), "q2".into(), "q3".into()],
            vec![q(1, 4), q(1, 2), q(1, 4)],
            &source_results,
            None,
            vec![format!("stage-ah-rational-{index}:mapping")],
        );
        receipts.push(receipt(
            format!("refused-rational-{index:03}"),
            Expected::Refused,
            bridge,
            source_results.iter().all(FormulaResult::replay_verified),
            None,
        ));
    }
    for index in 0..20 {
        let source_results = [1, 2, 3]
            .into_iter()
            .map(|quantity| source_result(&records, 5, quantity, &format!("invalid-prob-{index}")))
            .collect::<Vec<_>>();
        let bridge = bridge_source_scalars_to_expectation(
            vec!["q1".into(), "q2".into(), "q3".into()],
            vec![q(1, 2), q(1, 3), q(1, 3)],
            &source_results,
            None,
            vec![format!("stage-ah-invalid-probability-{index}:mapping")],
        );
        receipts.push(receipt(
            format!("refused-probability-{index:03}"),
            Expected::Refused,
            bridge,
            source_results.iter().all(FormulaResult::replay_verified),
            None,
        ));
    }
    for index in 0..20 {
        let source_results = vec![evaluate_formula_records(
            &the_machine::source_formula_pack::FormulaRequest {
                formula: "unknown_formula".into(),
                inputs: BTreeMap::new(),
                domain: DOMAIN.into(),
                ambiguity: None,
                provenance: vec![format!("stage-ah-missing-{index}")],
            },
            DOMAIN,
            &records,
        )];
        let bridge = bridge_source_scalars_to_expectation(
            vec!["q1".into()],
            vec![Rational::one()],
            &source_results,
            None,
            vec![format!("stage-ah-missing-{index}:mapping")],
        );
        receipts.push(receipt(
            format!("refused-source-{index:03}"),
            Expected::Refused,
            bridge,
            source_results.iter().all(FormulaResult::replay_verified),
            None,
        ));
    }
    for index in 0..20 {
        let source_results = [1, 2, 3]
            .into_iter()
            .map(|quantity| source_result(&records, 5, quantity, &format!("dimension-{index}")))
            .collect::<Vec<_>>();
        let bridge = bridge_source_scalars_to_expectation(
            vec!["q1".into(), "q2".into()],
            vec![q(1, 2), q(1, 2)],
            &source_results,
            None,
            vec![format!("stage-ah-dimension-{index}:mapping")],
        );
        receipts.push(receipt(
            format!("refused-dimension-{index:03}"),
            Expected::Refused,
            bridge,
            source_results.iter().all(FormulaResult::replay_verified),
            None,
        ));
    }
    let report = Report {
        schema: "stage-ah-source-probability-composition-v1",
        source_sha256: digest(SOURCE_DOCUMENT),
        source_record_count: records.len(),
        cases: receipts.len(),
        supported: 120,
        ambiguous: 40,
        refused: 80,
        exact_decisions: receipts.iter().filter(|r| r.exact).count(),
        complete_expectations: receipts
            .iter()
            .filter(|r| r.actual == SourceProbabilityBridgeStatus::Complete)
            .count(),
        source_replays: receipts.iter().filter(|r| r.source_replay).count(),
        bridge_replays: receipts.iter().filter(|r| r.bridge_replay).count(),
        bridge_tamper_rejections: receipts.iter().filter(|r| r.bridge_tamper_rejected).count(),
        value_correct: receipts.iter().filter(|r| r.value_correct).count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts
            .iter()
            .filter(|r| r.expected == Expected::Complete && !r.exact)
            .count(),
        live_mutations: 0,
        receipts,
    };
    assert_eq!(report.cases, 240);
    assert_eq!(report.exact_decisions, 240);
    assert_eq!(report.complete_expectations, 120);
    assert_eq!(report.source_replays, 240);
    assert_eq!(report.bridge_replays, 240);
    assert_eq!(report.bridge_tamper_rejections, 240);
    assert_eq!(report.value_correct, 240);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    std::fs::write(
        REPORT_MD,
        format!(
            "# Stage AH — source-derived probability composition\n\n| Measure | Result |\n| --- | ---: |\n| Cases | {} |\n| Supported / ambiguous / refused | {} / {} / {} |\n| Exact decisions | {}/{} |\n| Complete expectations | {} |\n| Source replay / bridge replay / tamper | {} / {} / {} |\n| Value checks | {}/{} |\n| False authorizations / denials | 0 / 0 |\n| Live mutation | 0 |\n\nThe bridge requires explicit outcome mapping, finite normalized probabilities, replayable source formulas, and integer-compatible values.\n\nReproduce with:\n\n```text\ncargo run --quiet --bin stage_ah_source_probability_composition\n```\n\nMachine-readable report: `{}`\n",
            report.cases,
            report.supported,
            report.ambiguous,
            report.refused,
            report.exact_decisions,
            report.cases,
            report.complete_expectations,
            report.source_replays,
            report.bridge_replays,
            report.bridge_tamper_rejections,
            report.value_correct,
            report.cases,
            REPORT_JSON,
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
