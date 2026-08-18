//! Stage 219: independent validation of the catalog-agnostic source-formula frontend.
//!
//! The corpus deliberately mixes two independently sourced catalogs.  The
//! frontend is given only catalog records and a domain string; it never sees
//! a subject-specific branch.  It must preserve complete, ambiguous, missing,
//! and unsupported outcomes before the generic source evaluator is invoked.

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_frontend::{
    formalize_source_formula_text, replay_verified, FrontendStatus,
};
use the_machine::source_formula_pack::{
    evaluate_formula_records, source_formula_records, FormulaRecord, FormulaStatus,
    InputConstraint,
};
use the_machine::source_statistics_pack;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
enum Expected {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone)]
struct Case {
    id: String,
    text: String,
    domain: String,
    records: Vec<FormulaRecord>,
    expected: Expected,
}

#[derive(Debug, serde::Serialize)]
struct Summary {
    schema: &'static str,
    cases: usize,
    expected_counts: BTreeMap<String, usize>,
    exact_status_decisions: usize,
    complete_frontends: usize,
    downstream_complete: usize,
    frontend_replays: usize,
    downstream_replays: usize,
    tamper_rejections: usize,
    provenance_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    corpus_sha256: String,
}

fn digest<T: serde::Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("benchmark rational is valid")
}

fn input_value(record: &FormulaRecord, name: &str) -> Rational {
    record
        .constraints
        .iter()
        .find_map(|constraint| match constraint {
            InputConstraint::Positive(input) if input == name => Some(q(2, 1)),
            InputConstraint::PositiveInteger(input) if input == name => Some(q(3, 1)),
            InputConstraint::NonnegativeInteger(input) if input == name => Some(q(4, 1)),
            InputConstraint::Probability(input) if input == name => Some(q(1, 4)),
            InputConstraint::NotEqualInteger(input, forbidden) if input == name => {
                Some(q(forbidden + 1, 1))
            }
            _ => None,
        })
        .unwrap_or_else(|| q(2, 1))
}

fn generated_text(record: &FormulaRecord, kind: Expected, index: usize) -> String {
    let alias = if index % 3 == 0 {
        record.aliases.first().map(String::as_str).unwrap_or(&record.formula_id)
    } else {
        &record.formula_id
    };
    let values: Vec<String> = record
        .required_inputs
        .iter()
        .enumerate()
        .filter(|(input_index, _)| !(kind == Expected::Missing && *input_index == 0))
        .map(|(_, name)| {
            let value = input_value(record, name);
            if value.denominator == 1 {
                format!("{name}={}", value.numerator)
            } else {
                format!("{name}={}/{}", value.numerator, value.denominator)
            }
        })
        .collect();
    let prefix = match kind {
        Expected::Ambiguous => "Choose the stated formula or an alternate formulation",
        Expected::Unsupported => "Give an approximate continuous result",
        _ => "Apply",
    };
    format!("{prefix} {alias}: {}", values.join(" and "))
}

fn make_cases() -> Vec<Case> {
    let catalogs = vec![
        (
            source_statistics_pack::DOMAIN.to_string(),
            source_statistics_pack::records(),
        ),
        (
            "source_derived_sequences_series".to_string(),
            source_formula_records(),
        ),
    ];
    let mut cases = Vec::new();
    for index in 0..1200usize {
        let (domain, records) = catalogs[index % catalogs.len()].clone();
        let record = &records[index % records.len()];
        let expected = match index % 10 {
            0..=6 => Expected::Complete,
            7 => Expected::Ambiguous,
            8 => Expected::Missing,
            _ => Expected::Unsupported,
        };
        cases.push(Case {
            id: format!("generic_formula_{index:04}"),
            text: generated_text(record, expected, index),
            domain,
            records,
            expected,
        });
    }
    cases
}

fn expected_status(expected: Expected) -> FrontendStatus {
    match expected {
        Expected::Complete => FrontendStatus::Complete,
        Expected::Ambiguous => FrontendStatus::Ambiguous,
        Expected::Missing => FrontendStatus::Missing,
        Expected::Unsupported => FrontendStatus::Unsupported,
    }
}

fn main() {
    let cases = make_cases();
    let corpus_sha256 = digest(
        &cases
            .iter()
            .map(|case| (&case.id, &case.text, &case.domain, case.expected))
            .collect::<Vec<_>>(),
    );
    let mut expected_counts = BTreeMap::new();
    let mut exact_status_decisions = 0;
    let mut complete_frontends = 0;
    let mut downstream_complete = 0;
    let mut frontend_replays = 0;
    let mut downstream_replays = 0;
    let mut tamper_rejections = 0;
    let mut provenance_preserved = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;

    for case in &cases {
        *expected_counts
            .entry(format!("{:?}", case.expected).to_ascii_lowercase())
            .or_insert(0) += 1;
        let frontend = formalize_source_formula_text(&case.text, &case.domain, &case.records);
        let expected = expected_status(case.expected);
        if frontend.status == expected {
            exact_status_decisions += 1;
        } else {
            false_denials += usize::from(case.expected == Expected::Complete);
            false_authorizations += usize::from(
                case.expected != Expected::Complete && frontend.status == FrontendStatus::Complete,
            );
        }
        if replay_verified(&frontend) {
            frontend_replays += 1;
        }
        if !frontend.provenance_spans.is_empty() {
            provenance_preserved += 1;
        }
        if case.expected == Expected::Complete && frontend.status == FrontendStatus::Complete {
            complete_frontends += 1;
            let request = frontend.request.as_ref().expect("complete frontend request");
            let execution = evaluate_formula_records(request, &case.domain, &case.records);
            if execution.status == FormulaStatus::Complete && execution.value.is_some() {
                downstream_complete += 1;
            }
            if execution.replay_verified() {
                downstream_replays += 1;
            }
        }
        let mut tampered = frontend.clone();
        tampered.replay_hash.push('x');
        if !replay_verified(&tampered) {
            tamper_rejections += 1;
        }
    }

    let summary = Summary {
        schema: "stage219-generic-source-formula-frontend-v1",
        cases: cases.len(),
        expected_counts,
        exact_status_decisions,
        complete_frontends,
        downstream_complete,
        frontend_replays,
        downstream_replays,
        tamper_rejections,
        provenance_preserved,
        false_authorizations,
        false_denials,
        corpus_sha256,
    };
    assert_eq!(summary.exact_status_decisions, summary.cases);
    assert_eq!(summary.complete_frontends, 840);
    assert_eq!(summary.downstream_complete, 840);
    assert_eq!(summary.frontend_replays, summary.cases);
    assert_eq!(summary.downstream_replays, 840);
    assert_eq!(summary.tamper_rejections, summary.cases);
    assert_eq!(summary.provenance_preserved, summary.cases);
    assert_eq!(summary.false_authorizations, 0);
    assert_eq!(summary.false_denials, 0);
    println!("{}", serde_json::to_string_pretty(&summary).unwrap());
}
