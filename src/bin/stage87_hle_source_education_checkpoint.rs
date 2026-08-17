//! Stage 87: frozen HLE diagnostic after source-derived education.
//!
//! This checkpoint never authorizes an HLE answer.  It measures only whether
//! the two source-backed language frontends can safely reach their typed
//! catalogs on the untouched HLE export.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, source_formula_records, FormulaStatus,
};
use the_machine::source_sequence_frontend::{
    formalize_sequence_text, replay_verified as sequence_replay, SequenceFrontendStatus,
};
use the_machine::source_unit_frontend::{
    formalize_unit_text, replay_verified as unit_replay, UnitFrontendStatus,
};

const HLE: &str = "data/hle.jsonl";
const UNIT_SOURCE: &str = include_str!("../../docs/sources/openstax_unit_conversion_catalog.txt");
const DOMAIN_SEQUENCE: &str = "source_catalog_sequences_series";
const DOMAIN_UNIT: &str = "source_catalog_unit_conversion";
const REPORT_JSON: &str = "docs/stage87_hle_source_education_checkpoint.json";
const REPORT_MD: &str = "docs/stage87_hle_source_education_checkpoint.md";

#[derive(Debug, Deserialize)]
struct HleRow {
    id: String,
    question: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    hle_sha256: String,
    cases: usize,
    sequence_frontend_complete: usize,
    sequence_pack_complete: usize,
    unit_frontend_complete: usize,
    unit_pack_complete: usize,
    both_frontends_complete: usize,
    frontend_ambiguities: usize,
    unsupported_or_missing: usize,
    frontend_replays: usize,
    frontend_tamper_rejections: usize,
    potential_single_routes_not_authorized: usize,
    false_authorizations: usize,
    production_mutations: usize,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hle_bytes = fs::read(HLE)?;
    let hle_text = String::from_utf8(hle_bytes.clone())?;
    let rows: Vec<HleRow> = hle_text
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let sequence_records = source_formula_records();
    let unit_records = extract_formula_records(UNIT_SOURCE).map_err(|errors| errors.join("; "))?;
    let mut sequence_frontend_complete = 0;
    let mut sequence_pack_complete = 0;
    let mut unit_frontend_complete = 0;
    let mut unit_pack_complete = 0;
    let mut both_frontends_complete = 0;
    let mut frontend_ambiguities = 0;
    let mut unsupported_or_missing = 0;
    let mut frontend_replays = 0;
    let mut frontend_tamper_rejections = 0;
    let mut potential_single_routes_not_authorized = 0;
    for row in &rows {
        let sequence = formalize_sequence_text(&row.question, &format!("hle-sequence-{}", row.id));
        let unit = formalize_unit_text(
            &row.question,
            &format!("hle-unit-{}", row.id),
            &unit_records,
        );
        frontend_replays += usize::from(sequence_replay(&sequence) && unit_replay(&unit));
        let mut sequence_tampered = sequence.clone();
        sequence_tampered.replay_hash.push('x');
        let mut unit_tampered = unit.clone();
        unit_tampered.replay_hash.push('x');
        frontend_tamper_rejections +=
            usize::from(!sequence_replay(&sequence_tampered) && !unit_replay(&unit_tampered));
        let sequence_complete = sequence.status == SequenceFrontendStatus::Complete;
        let sequence_pack = sequence.request.as_ref().is_some_and(|request| {
            evaluate_formula_records(request, DOMAIN_SEQUENCE, &sequence_records).status
                == FormulaStatus::Complete
        });
        let unit_complete = unit.status == UnitFrontendStatus::Complete;
        let unit_pack = unit.request.as_ref().is_some_and(|request| {
            evaluate_formula_records(request, DOMAIN_UNIT, &unit_records).status
                == FormulaStatus::Complete
        });
        sequence_frontend_complete += usize::from(sequence_complete);
        sequence_pack_complete += usize::from(sequence_pack);
        unit_frontend_complete += usize::from(unit_complete);
        unit_pack_complete += usize::from(unit_pack);
        both_frontends_complete += usize::from(sequence_pack && unit_pack);
        frontend_ambiguities += usize::from(
            sequence.status == SequenceFrontendStatus::Ambiguous
                || unit.status == UnitFrontendStatus::Ambiguous,
        );
        unsupported_or_missing += usize::from(
            !sequence_complete
                && !unit_complete
                && sequence.status != SequenceFrontendStatus::Ambiguous
                && unit.status != UnitFrontendStatus::Ambiguous,
        );
        potential_single_routes_not_authorized += usize::from((sequence_pack ^ unit_pack));
    }
    assert_eq!(frontend_replays, rows.len());
    assert_eq!(frontend_tamper_rejections, rows.len());
    let report = Report {
        schema: "stage87-hle-source-education-checkpoint-v1",
        hle_sha256: digest_bytes(&hle_bytes),
        cases: rows.len(),
        sequence_frontend_complete,
        sequence_pack_complete,
        unit_frontend_complete,
        unit_pack_complete,
        both_frontends_complete,
        frontend_ambiguities,
        unsupported_or_missing,
        frontend_replays,
        frontend_tamper_rejections,
        potential_single_routes_not_authorized,
        false_authorizations: 0,
        production_mutations: 0,
    };
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(REPORT_MD, format!(
        "# Stage 87 — frozen HLE source-education checkpoint\n\n- HLE cases: {}\n- Sequence frontend/pack complete: {}/{}\n- Unit frontend/pack complete: {}/{}\n- Both complete: {}\n- Frontend ambiguities: {}\n- Unsupported/missing: {}\n- Frontend replay/tamper: {}/{}\n- Potential single routes (not authorized): {}\n- False authorizations / production mutations: {} / {}\n- HLE SHA-256: `{}`\n",
        report.cases, report.sequence_frontend_complete, report.sequence_pack_complete, report.unit_frontend_complete, report.unit_pack_complete, report.both_frontends_complete, report.frontend_ambiguities, report.unsupported_or_missing, report.frontend_replays, report.frontend_tamper_rejections, report.potential_single_routes_not_authorized, report.false_authorizations, report.production_mutations, report.hle_sha256,
    ))?;
    Ok(())
}
