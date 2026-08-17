//! Stage 85: source-derived unit-conversion frontend and external scan.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, validate_formula_records, FormulaStatus,
};
use the_machine::source_unit_frontend::{formalize_unit_text, replay_verified, UnitFrontendStatus};
use the_machine::third_party_corpus_benchmark::ThirdPartyCorpus;

const SOURCE: &str = include_str!("../../docs/sources/openstax_unit_conversion_catalog.txt");
const DOMAIN: &str = "source_catalog_unit_conversion";
const RELEASE: &str = "data/third_party_gsm8k_restricted_release_v2.json";
const REPORT_JSON: &str = "docs/stage85_unit_conversion_frontend_bench.json";
const REPORT_MD: &str = "docs/stage85_unit_conversion_frontend_bench.md";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_sha256: String,
    records: usize,
    source_valid: bool,
    development_cases: usize,
    development_exact: usize,
    development_frontend_replay: usize,
    development_execution_replay: usize,
    development_tamper_rejection: usize,
    development_values: usize,
    false_authorizations: usize,
    false_denials: usize,
    holdout_cases: usize,
    holdout_exact: usize,
    holdout_values: usize,
    holdout_replays: usize,
    holdout_tamper_rejections: usize,
    external_cases: usize,
    external_complete: usize,
    external_pack_complete: usize,
    external_ambiguous: usize,
    external_unsupported_or_missing: usize,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn unit_text(index: usize, pair: usize) -> String {
    let amount = 2 + index % 9;
    match pair {
        0 => format!("Convert {amount} meters to centimeters using the catalog relation."),
        1 => format!("Express {amount} hours into minutes using the stated source relation."),
        2 => format!("Convert {amount} pounds as ounces using the catalog relation."),
        _ => format!("Convert {amount} liters to milliliters using the source relation."),
    }
}

fn expected_value(pair: usize, index: usize) -> i128 {
    let amount = (2 + index % 9) as i128;
    amount * [100, 60, 16, 1000][pair]
}

fn run_case(
    records: &[the_machine::source_formula_pack::FormulaRecord],
    expected: Expected,
    index: usize,
    pair: usize,
    text: &str,
) -> (bool, bool, bool, bool, bool, bool) {
    let frontend = formalize_unit_text(text, &format!("stage85-{index}"), records);
    let frontend_exact = match expected {
        Expected::Supported => frontend.status == UnitFrontendStatus::Complete,
        Expected::Ambiguous => frontend.status == UnitFrontendStatus::Ambiguous,
        Expected::Unsupported => matches!(
            frontend.status,
            UnitFrontendStatus::Unsupported | UnitFrontendStatus::Missing
        ),
    };
    let frontend_replay = replay_verified(&frontend);
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let frontend_tamper = !replay_verified(&frontend_tampered);
    let mut execution_replay = true;
    let mut execution_tamper = true;
    let mut value_correct = expected != Expected::Supported;
    let execution_exact = if let Some(request) = frontend.request.as_ref() {
        let result = evaluate_formula_records(request, DOMAIN, records);
        execution_replay = result.replay_verified();
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        execution_tamper = !tampered.replay_verified();
        value_correct = result.status == FormulaStatus::Complete
            && result.value.as_ref().is_some_and(|value| {
                value.numerator == expected_value(pair, index) && value.denominator == 1
            });
        expected == Expected::Supported && value_correct
    } else {
        expected != Expected::Supported
    };
    (
        frontend_exact && execution_exact,
        frontend_replay,
        execution_replay,
        frontend_tamper && execution_tamper,
        value_correct,
        (expected != Expected::Supported
            && frontend.status == UnitFrontendStatus::Complete
            && execution_exact)
            || (expected == Expected::Supported && !(frontend_exact && execution_exact)),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let records = extract_formula_records(SOURCE).map_err(|errors| errors.join("; "))?;
    assert_eq!(records.len(), 4);
    assert!(validate_formula_records(&records).is_ok());
    let mut exact = 0;
    let mut frontend_replay = 0;
    let mut execution_replay = 0;
    let mut tamper = 0;
    let mut values = 0;
    let mut false_auth = 0;
    let mut false_deny = 0;
    for index in 0..600 {
        let pair = index % 4;
        let result = run_case(
            &records,
            Expected::Supported,
            index,
            pair,
            &unit_text(index, pair),
        );
        exact += usize::from(result.0);
        frontend_replay += usize::from(result.1);
        execution_replay += usize::from(result.2);
        tamper += usize::from(result.3);
        values += usize::from(result.4);
        false_deny += usize::from(result.5);
    }
    for index in 0..200 {
        let result = run_case(
            &records,
            Expected::Ambiguous,
            index + 600,
            0,
            "Convert 3 meters to centimeters or millimeters.",
        );
        exact += usize::from(result.0);
        frontend_replay += usize::from(result.1);
        execution_replay += usize::from(result.2);
        tamper += usize::from(result.3);
        values += usize::from(result.4);
        false_auth += usize::from(result.5);
    }
    for index in 0..100 {
        let result = run_case(
            &records,
            Expected::Unsupported,
            index + 800,
            0,
            "Convert 3 yards to centimeters using the catalog relation.",
        );
        exact += usize::from(result.0);
        frontend_replay += usize::from(result.1);
        execution_replay += usize::from(result.2);
        tamper += usize::from(result.3);
        values += usize::from(result.4);
        false_auth += usize::from(result.5);
    }
    for index in 0..100 {
        let result = run_case(
            &records,
            Expected::Unsupported,
            index + 900,
            0,
            "Convert an amount to centimeters without stating the source unit.",
        );
        exact += usize::from(result.0);
        frontend_replay += usize::from(result.1);
        execution_replay += usize::from(result.2);
        tamper += usize::from(result.3);
        values += usize::from(result.4);
        false_auth += usize::from(result.5);
    }
    let mut holdout_exact = 0;
    let mut holdout_values = 0;
    let mut holdout_replay = 0;
    let mut holdout_tamper = 0;
    for index in 0..200 {
        let pair = (index + 2) % 4;
        let result = run_case(
            &records,
            Expected::Supported,
            index + 2_000,
            pair,
            &unit_text(index + 2_000, pair),
        );
        holdout_exact += usize::from(result.0);
        holdout_values += usize::from(result.4);
        holdout_replay += usize::from(result.1 && result.2);
        holdout_tamper += usize::from(result.3);
    }
    assert_eq!(
        (exact, frontend_replay, execution_replay, tamper, values),
        (1_000, 1_000, 1_000, 1_000, 1_000)
    );
    assert_eq!((false_auth, false_deny), (0, 0));
    assert_eq!(
        (
            holdout_exact,
            holdout_values,
            holdout_replay,
            holdout_tamper
        ),
        (200, 200, 200, 200)
    );
    let external: ThirdPartyCorpus = serde_json::from_slice(&fs::read(RELEASE)?)?;
    let mut external_complete = 0;
    let mut external_pack_complete = 0;
    let mut external_ambiguous = 0;
    for case in &external.cases {
        let frontend = formalize_unit_text(&case.original_prompt, &case.id, &records);
        if frontend.status == UnitFrontendStatus::Complete {
            external_complete += 1;
            if frontend.request.as_ref().is_some_and(|request| {
                evaluate_formula_records(request, DOMAIN, &records).status
                    == FormulaStatus::Complete
            }) {
                external_pack_complete += 1;
            }
        } else if frontend.status == UnitFrontendStatus::Ambiguous {
            external_ambiguous += 1;
        }
    }
    let report = Report {
        schema: "stage85-unit-conversion-frontend-bench-v1",
        source_sha256: digest(&SOURCE),
        records: records.len(),
        source_valid: true,
        development_cases: 1_000,
        development_exact: exact,
        development_frontend_replay: frontend_replay,
        development_execution_replay: execution_replay,
        development_tamper_rejection: tamper,
        development_values: values,
        false_authorizations: false_auth,
        false_denials: false_deny,
        holdout_cases: 200,
        holdout_exact,
        holdout_values,
        holdout_replays: holdout_replay,
        holdout_tamper_rejections: holdout_tamper,
        external_cases: external.cases.len(),
        external_complete,
        external_pack_complete,
        external_ambiguous,
        external_unsupported_or_missing: external.cases.len()
            - external_complete
            - external_ambiguous,
    };
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(REPORT_MD, format!(
        "# Stage 85 — source unit-conversion frontend\n\n- Catalog records: {}, valid: {}\n- Development: {}/{} exact, frontend/execution replay {}/{}, tamper {}, values {}\n- Holdout: {}/{} exact, values {}, replay {}, tamper {}\n- False authorizations / denials: {} / {}\n- External release: {} cases, {} frontend-complete, {} pack-complete, {} ambiguous, {} unsupported/missing\n- Source SHA-256: `{}`\n",
        report.records, report.source_valid, report.development_exact, report.development_cases, report.development_frontend_replay, report.development_execution_replay, report.development_tamper_rejection, report.development_values, report.holdout_exact, report.holdout_cases, report.holdout_values, report.holdout_replays, report.holdout_tamper_rejections, report.false_authorizations, report.false_denials, report.external_cases, report.external_complete, report.external_pack_complete, report.external_ambiguous, report.external_unsupported_or_missing, report.source_sha256,
    ))?;
    Ok(())
}
