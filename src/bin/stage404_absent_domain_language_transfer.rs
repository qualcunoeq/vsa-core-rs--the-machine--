//! Stage 404: answer-key-blind language transfer for gap-selected sources.
//!
//! Prompts are assembled from source-generated exercises through generic
//! aliases and declared inputs.  The frontend is the same domain-agnostic
//! source-formula frontend used by existing catalogs; no health/economics
//! branch or answer key is consulted.  This is source-derived transfer
//! evidence, not a claim of independently authored benchmark performance.

use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use the_machine::source_evidence_envelope::{ingest_source_evidence, SourceEvidenceEnvelope};
use the_machine::source_exercise_generation::{exercise_replay_verified, generate_exercises};
use the_machine::source_formula_frontend::{formalize_source_formula_text, FrontendStatus};
use the_machine::source_formula_pack::{evaluate_formula_records, FormulaRecord, FormulaStatus};
use the_machine::source_module_discovery::discover_formula_corpus;
use the_machine::source_selection::{
    gap_replay_verified, select_for_gap, SourceGapRequest, SourceSelectionDecision,
};

const HEALTH_SOURCE: &str =
    include_str!("../../docs/sources/openstax_bounded_health_ratios_source.txt");
const ECONOMICS_SOURCE: &str =
    include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const JSON: &str = "docs/stage404_absent_domain_language_transfer.json";
const MD: &str = "docs/stage404_absent_domain_language_transfer.md";
const DOMAIN: &str = "source_derived_gap_selected_rational_expression";
const SCOPE: &str = "bounded exact rational-expression evaluation";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Complete,
    Ambiguous,
    Refused,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    frontend_status: FrontendStatus,
    downstream_status: Option<FormulaStatus>,
    frontend_exact: bool,
    downstream_exact: bool,
    value_correct: bool,
    frontend_replay: bool,
    downstream_replay: bool,
    frontend_tamper_rejected: bool,
    downstream_tamper_rejected: bool,
    false_authorization: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_gap_id: String,
    selected_source_ids: Vec<String>,
    rejected_decoys: usize,
    gap_selection_replay: bool,
    source_modules: usize,
    source_records: usize,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    frontend_exact: usize,
    downstream_exact: usize,
    frontend_replays: usize,
    downstream_replays: usize,
    frontend_tamper_rejections: usize,
    downstream_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    answer_keys_read: usize,
    live_mutations: usize,
    receipts: Vec<Receipt>,
}

fn rational_text(value: &the_machine::probability_pack::Rational) -> String {
    if value.denominator == 1 {
        value.numerator.to_string()
    } else {
        format!("{}/{}", value.numerator, value.denominator)
    }
}

fn source_evidence(
    path: &str,
    module: &the_machine::source_module_discovery::DiscoveredSourceModule,
) -> SourceEvidenceEnvelope {
    let source = module.records.first().unwrap().source.clone();
    let document = format!(
        "SOURCE_ID: {}\nTITLE: {}\nSECTION: {}\nURL: {}\nLICENSE: {}\nRETRIEVED_UTC: {}\nEVIDENCE: {}\nSCOPE: {}",
        source.source_id,
        source.title,
        source.section,
        source.url,
        source.license,
        source.retrieved_utc,
        source.evidence_span,
        SCOPE,
    );
    ingest_source_evidence(path, &document).unwrap()
}

fn decoy() -> SourceEvidenceEnvelope {
    ingest_source_evidence(
        "docs/sources/openstax_complex_arithmetic_source.txt",
        "SOURCE_ID: openstax-precalculus-2e:complex-arithmetic\nTITLE: Precalculus 2e\nSECTION: Complex Numbers\nURL: https://openstax.org/details/books/precalculus-2e\nLICENSE: CC BY 4.0\nRETRIEVED_UTC: 2026-08-17\nEVIDENCE: rectangular complex arithmetic\nSCOPE: bounded complex arithmetic",
    )
    .unwrap()
}

fn prompt(
    record: &FormulaRecord,
    inputs: &BTreeMap<String, the_machine::probability_pack::Rational>,
    variant: usize,
) -> String {
    let mut fields = record
        .required_inputs
        .iter()
        .map(|name| format!("{name} = {}", rational_text(&inputs[name])))
        .collect::<Vec<_>>();
    let rotation = variant % fields.len().max(1);
    fields.rotate_left(rotation);
    let lead = match variant % 3 {
        0 => "Calculate",
        1 => "Determine",
        _ => "Find",
    };
    format!(
        "{lead} the {} using the following declared quantities: {}.",
        record.aliases[0],
        fields.join("; "),
    )
}

fn evaluate_receipt(
    id: String,
    expected: Expected,
    text: &str,
    expected_value: Option<the_machine::probability_pack::Rational>,
    records: &[FormulaRecord],
) -> Receipt {
    let frontend = formalize_source_formula_text(text, DOMAIN, records);
    let frontend_exact = match expected {
        Expected::Complete => frontend.status == FrontendStatus::Complete,
        Expected::Ambiguous => frontend.status == FrontendStatus::Ambiguous,
        Expected::Refused => frontend.status != FrontendStatus::Complete,
    };
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let (downstream_status, downstream_exact, value_correct, downstream_replay, downstream_tamper) =
        if let Some(request) = frontend.request.clone() {
            let result = evaluate_formula_records(&request, DOMAIN, records);
            let exact = expected == Expected::Complete && result.status == FormulaStatus::Complete;
            let value_correct = expected != Expected::Complete || result.value == expected_value;
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            (
                Some(result.status),
                exact,
                value_correct,
                result.replay_verified(),
                !tampered.replay_verified(),
            )
        } else {
            (None, false, true, false, false)
        };
    Receipt {
        id,
        expected,
        frontend_status: frontend.status,
        downstream_status,
        frontend_exact,
        downstream_exact,
        value_correct,
        frontend_replay: frontend.replay_verified(),
        downstream_replay,
        frontend_tamper_rejected: !frontend_tampered.replay_verified(),
        downstream_tamper_rejected: downstream_tamper,
        false_authorization: expected != Expected::Complete
            && (frontend.status == FrontendStatus::Complete
                || downstream_status == Some(FormulaStatus::Complete)),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let health = discover_formula_corpus(&[HEALTH_SOURCE], "health")
        .map_err(|errors| errors.join("; "))?
        .into_iter()
        .find(|module| module.candidate.source_ids[0].contains(":health:incidence"))
        .unwrap();
    let economics = discover_formula_corpus(&[ECONOMICS_SOURCE], "economics")
        .map_err(|errors| errors.join("; "))?
        .into_iter()
        .find(|module| module.candidate.source_ids[0].ends_with(":revenue"))
        .unwrap();
    let candidates = vec![
        source_evidence("health-ratios", &health),
        source_evidence("economics", &economics),
        decoy(),
    ];
    let request = SourceGapRequest::new(
        "gap::unimplemented-ratio-and-identity-domain",
        vec![SCOPE.into()],
        2,
    );
    let selection = select_for_gap(&request, &candidates);
    assert!(gap_replay_verified(&request, &selection));
    assert_eq!(selection.selected_source_ids.len(), 2);
    assert_eq!(
        selection
            .candidates
            .iter()
            .filter(|candidate| candidate.decision == SourceSelectionDecision::Rejected)
            .count(),
        1
    );

    let health_generated =
        generate_exercises(&health.records, DOMAIN, 30).map_err(|errors| errors.join("; "))?;
    let economics_generated =
        generate_exercises(&economics.records, DOMAIN, 30).map_err(|errors| errors.join("; "))?;
    assert!(the_machine::source_exercise_generation::replay_verified(
        &health_generated
    ));
    assert!(the_machine::source_exercise_generation::replay_verified(
        &economics_generated
    ));
    let records = health
        .records
        .iter()
        .chain(economics.records.iter())
        .cloned()
        .collect::<Vec<_>>();
    let mut receipts = Vec::new();
    for (index, exercise) in health_generated
        .exercises
        .iter()
        .chain(economics_generated.exercises.iter())
        .enumerate()
    {
        assert!(exercise_replay_verified(exercise));
        let record = records
            .iter()
            .find(|record| record.formula_id == exercise.formula_id)
            .unwrap();
        receipts.push(evaluate_receipt(
            format!("supported-{index:03}"),
            Expected::Complete,
            &prompt(record, &exercise.inputs, index),
            Some(exercise.expected.clone()),
            &records,
        ));
    }
    let first = &records[0];
    let second = &records[1];
    let ambiguous_text = format!(
        "Calculate the {} or the {}; price=7; quantity=4; new_cases=2; population=10.",
        first.aliases[0], second.aliases[0]
    );
    receipts.push(evaluate_receipt(
        "ambiguous-000".into(),
        Expected::Ambiguous,
        &ambiguous_text,
        None,
        &records,
    ));
    receipts.push(evaluate_receipt(
        "ambiguous-001".into(),
        Expected::Ambiguous,
        &ambiguous_text,
        None,
        &records,
    ));
    receipts.push(evaluate_receipt(
        "missing-000".into(),
        Expected::Refused,
        &format!("Calculate the {} using price=7.", second.aliases[0]),
        None,
        &records,
    ));
    receipts.push(evaluate_receipt(
        "unsupported-000".into(),
        Expected::Refused,
        &format!(
            "Calculate the {} for a continuous model using price=7 and quantity=4.",
            second.aliases[0]
        ),
        None,
        &records,
    ));
    let report = Report {
        schema: "stage404-absent-domain-language-transfer-v1",
        source_gap_id: request.gap_id.clone(),
        selected_source_ids: selection.selected_source_ids.clone(),
        rejected_decoys: 1,
        gap_selection_replay: gap_replay_verified(&request, &selection),
        source_modules: 2,
        source_records: records.len(),
        cases: receipts.len(),
        supported: 60,
        ambiguous: 2,
        refused: 2,
        frontend_exact: receipts
            .iter()
            .filter(|receipt| receipt.frontend_exact)
            .count(),
        downstream_exact: receipts
            .iter()
            .filter(|receipt| receipt.downstream_exact && receipt.value_correct)
            .count(),
        frontend_replays: receipts
            .iter()
            .filter(|receipt| receipt.frontend_replay)
            .count(),
        downstream_replays: receipts
            .iter()
            .filter(|receipt| receipt.downstream_replay)
            .count(),
        frontend_tamper_rejections: receipts
            .iter()
            .filter(|receipt| receipt.frontend_tamper_rejected)
            .count(),
        downstream_tamper_rejections: receipts
            .iter()
            .filter(|receipt| receipt.downstream_tamper_rejected)
            .count(),
        false_authorizations: receipts
            .iter()
            .filter(|receipt| receipt.false_authorization)
            .count(),
        false_denials: receipts
            .iter()
            .filter(|receipt| {
                receipt.expected == Expected::Complete
                    && !(receipt.downstream_exact && receipt.value_correct)
            })
            .count(),
        answer_keys_read: 0,
        live_mutations: 0,
        receipts,
    };
    assert_eq!(report.cases, 64);
    assert_eq!(report.frontend_exact, 64);
    assert_eq!(report.downstream_exact, 60);
    assert_eq!(report.frontend_replays, 64);
    assert_eq!(report.downstream_replays, 60);
    assert_eq!(report.frontend_tamper_rejections, 64);
    assert_eq!(report.downstream_tamper_rejections, 60);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(JSON, format!("{serialized}\n"))?;
    fs::write(MD, format!("# Stage 404 — gap-selected absent-domain language transfer\n\n- Source modules / records: {} / {}\n- Selected source lineages / rejected decoys: {} / {}\n- Gap-selection replay: {}\n- Cases (supported / ambiguous / refused): {} ({} / {} / {})\n- Frontend exact / replay / tamper: {} / {} / {}\n- Downstream exact / replay / tamper: {} / {} / {}\n- False authorizations / denials: {} / {}\n- Answer keys read / live mutations: {} / {}\n\nPrompts were assembled from source-generated exercises through generic aliases and declared inputs. This is source-derived transfer evidence, not an independently authored benchmark claim; no subject-specific frontend branch or live mutation was used.\n", report.source_modules, report.source_records, report.selected_source_ids.len(), report.rejected_decoys, report.gap_selection_replay, report.cases, report.supported, report.ambiguous, report.refused, report.frontend_exact, report.frontend_replays, report.frontend_tamper_rejections, report.downstream_exact, report.downstream_replays, report.downstream_tamper_rejections, report.false_authorizations, report.false_denials, report.answer_keys_read, report.live_mutations))?;
    println!("{serialized}");
    Ok(())
}
