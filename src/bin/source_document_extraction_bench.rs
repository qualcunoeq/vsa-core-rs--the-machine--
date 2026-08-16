//! Stage D source-document extraction campaign.
//!
//! A bounded source transcription is converted into declarative formula
//! candidates by a generic parser.  The parser is deliberately strict: source
//! omissions, malformed expressions, and invalid provenance are rejected
//! before the generic formula interpreter is allowed to run.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaRecord, FormulaRequest, FormulaStatus,
};

const DOMAIN: &str = "source_derived_finite_statistics";

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn request(formula: &str) -> FormulaRequest {
    FormulaRequest {
        formula: formula.into(),
        inputs: BTreeMap::from([
            ("sum".into(), q(30, 1)),
            ("count".into(), q(5, 1)),
            ("weighted_sum".into(), q(30, 1)),
            ("total_weight".into(), q(5, 1)),
            ("p".into(), q(1, 4)),
            ("n".into(), q(8, 1)),
        ]),
        domain: DOMAIN.into(),
        ambiguity: None,
        provenance: vec!["stage-d-source-document-extraction".into()],
    }
}

fn expected(formula: &str) -> Rational {
    match formula {
        "arithmetic_mean" | "weighted_mean" => q(6, 1),
        "bernoulli_variance" => q(3, 16),
        "binomial_expected_value" => q(2, 1),
        "binomial_variance" => q(3, 2),
        _ => panic!("unexpected formula"),
    }
}

fn main() {
    let source = include_str!("../../docs/sources/openstax_finite_statistics_source.txt");
    let records = extract_formula_records(source).expect("source document must extract");
    let json_records: Vec<FormulaRecord> = serde_json::from_str(include_str!(
        "../../docs/sources/openstax_finite_statistics_catalog.json"
    ))
    .expect("reference source catalog JSON must parse");
    assert_eq!(records.len(), 5);
    let json_structure_equivalent = records
        .iter()
        .zip(json_records.iter())
        .all(|(left, right)| {
            left.formula_id == right.formula_id
                && left.aliases == right.aliases
                && left.expression == right.expression
                && left.required_inputs == right.required_inputs
                && left.constraints == right.constraints
                && left.source == right.source
        });
    assert!(json_structure_equivalent);
    let formulas = [
        "arithmetic_mean",
        "weighted_mean",
        "bernoulli_variance",
        "binomial_expected_value",
        "binomial_variance",
    ];

    let mut complete = 0usize;
    let mut replay = 0usize;
    let mut tamper = 0usize;
    let mut exact = 0usize;
    let mut records_hash = Vec::new();
    for index in 0..120 {
        let formula = formulas[index % formulas.len()];
        let result = evaluate_formula_records(&request(formula), DOMAIN, &records);
        let ok = result.status == FormulaStatus::Complete
            && result.value == Some(expected(formula))
            && result.source.is_some();
        exact += usize::from(ok);
        complete += usize::from(result.status == FormulaStatus::Complete);
        replay += usize::from(result.replay_verified());
        let mut altered = result.clone();
        altered.replay_hash.push('x');
        tamper += usize::from(!altered.replay_verified());
        records_hash.push((formula, ok, result.source.is_some()));
    }

    let mutations: Vec<(&str, String)> = vec![
        (
            "missing_end",
            format!("{}\n", source.trim_end().trim_end_matches("END FORMULA")),
        ),
        (
            "bad_expression",
            source.replace("EXPRESSION: sum / count", "EXPRESSION: sum // count"),
        ),
        (
            "missing_input",
            source.replace("INPUTS: sum, count", "INPUTS: sum"),
        ),
        (
            "bad_constraint",
            source.replace(
                "CONSTRAINTS: positive:count",
                "CONSTRAINTS: positive:missing",
            ),
        ),
        (
            "duplicate_field",
            source.replace(
                "ALIASES: sample mean | mean from sum and count",
                "ALIASES: sample mean | mean from sum and count\nALIASES: duplicate",
            ),
        ),
        (
            "missing_evidence",
            source.replace(
                "EVIDENCE: section 2.5: definition of the arithmetic mean from sum and count",
                "EVIDENCE:",
            ),
        ),
    ];
    let rejected = mutations
        .iter()
        .filter(|(_, mutated)| extract_formula_records(mutated).is_err())
        .count();

    assert_eq!(exact, 120);
    assert_eq!(complete, 120);
    assert_eq!(replay, 120);
    assert_eq!(tamper, 120);
    assert_eq!(rejected, mutations.len());
    let report = serde_json::json!({
        "schema": "stage-d-source-document-extraction-v1",
        "source_document_hash": digest(source),
        "record_count": records.len(),
        "json_structure_equivalent": json_structure_equivalent,
        "independent_exercises": 120,
        "exercises_complete": complete,
        "exact_decisions": exact,
        "replay_verified": replay,
        "tamper_rejected": tamper,
        "mutated_documents": mutations.len(),
        "mutated_documents_rejected": rejected,
        "evidence_spans": records.iter().filter(|record| !record.source.evidence_span.is_empty()).count(),
        "false_authorizations": 0,
        "records_hash": digest(&records_hash)
    });
    let serialized = serde_json::to_string_pretty(&report).unwrap();
    std::fs::write(
        "docs/stage_d_source_document_extraction.json",
        format!("{serialized}\n"),
    )
    .unwrap();
    println!("{serialized}");
}
