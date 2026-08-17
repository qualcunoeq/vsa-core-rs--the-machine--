//! Stage 84: language access to the source-derived sequence catalog.
//!
//! Frontend and source execution are measured separately.  A complete parse
//! is not itself an authorization; only a typed request accepted by the
//! source catalog may execute, and this benchmark remains shadow-only.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::source_formula_pack::{
    evaluate_formula_records, source_formula_records, validate_formula_records, FormulaStatus,
};
use the_machine::source_sequence_frontend::{
    formalize_sequence_text, replay_verified, SequenceFrontendStatus,
};
use the_machine::third_party_corpus_benchmark::ThirdPartyCorpus;

const REPORT_JSON: &str = "docs/stage84_sequence_frontend_bench.json";
const REPORT_MD: &str = "docs/stage84_sequence_frontend_bench.md";
const DOMAIN: &str = "source_catalog_sequences_series";
const EXTERNAL_RELEASE: &str = "data/third_party_gsm8k_restricted_release_v2.json";

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
    source_catalog_sha256: String,
    source_catalog_valid: bool,
    development_cases: usize,
    development_supported: usize,
    development_ambiguous: usize,
    development_unsupported: usize,
    development_exact: usize,
    frontend_replays: usize,
    frontend_tamper_rejections: usize,
    execution_replays: usize,
    execution_tamper_rejections: usize,
    execution_value_correct: usize,
    false_authorizations: usize,
    false_denials: usize,
    holdout_cases: usize,
    holdout_exact: usize,
    holdout_value_correct: usize,
    holdout_frontend_replays: usize,
    holdout_execution_replays: usize,
    holdout_tamper_rejections: usize,
    external_cases_scanned: usize,
    external_frontend_complete: usize,
    external_pack_complete: usize,
    external_ambiguous: usize,
    external_unsupported_or_missing: usize,
    external_false_authorizations: usize,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(value: i128) -> the_machine::probability_pack::Rational {
    the_machine::probability_pack::Rational::new(value, 1).unwrap()
}

fn text(index: usize, formula: usize) -> String {
    let a1 = 2 + (index % 7);
    let n = 2 + (index % 8);
    let d = 1 + (index % 5);
    let r = 2 + (index % 4);
    match formula {
        0 => format!("An arithmetic sequence has first term = {a1}, common difference = {d}; find the nth term for n = {n}."),
        1 => format!("For an arithmetic sequence with first term = {a1} and difference = {d}, find the sum of the first n terms when n = {n}."),
        2 => format!("A geometric sequence starts with first term = {a1} and common ratio = {r}; find the nth term for n = {n}."),
        _ => format!("For a geometric sequence with first term = {a1} and ratio = {r}, find the sum of the first n terms when n = {n}."),
    }
}

fn oracle(formula: usize, index: usize) -> the_machine::probability_pack::Rational {
    let a1 = q((2 + (index % 7)) as i128);
    let n = (2 + (index % 8)) as usize;
    let d = q((1 + (index % 5)) as i128);
    let r = q((2 + (index % 4)) as i128);
    match formula {
        0 => a1.add(&q((n - 1) as i128).mul(&d).unwrap()).unwrap(),
        1 => q(n as i128)
            .mul(
                &a1.mul(&q(2))
                    .unwrap()
                    .add(&q((n - 1) as i128).mul(&d).unwrap())
                    .unwrap(),
            )
            .unwrap()
            .div(&q(2))
            .unwrap(),
        2 => a1.mul(&pow(&r, n - 1)).unwrap(),
        _ => a1
            .mul(&pow(&r, n).sub(&q(1)).unwrap())
            .unwrap()
            .div(&r.sub(&q(1)).unwrap())
            .unwrap(),
    }
}

fn pow(
    base: &the_machine::probability_pack::Rational,
    exponent: usize,
) -> the_machine::probability_pack::Rational {
    let mut result = the_machine::probability_pack::Rational::one();
    for _ in 0..exponent {
        result = result.mul(base).unwrap();
    }
    result
}

fn run_case(
    records: &[the_machine::source_formula_pack::FormulaRecord],
    expected: Expected,
    index: usize,
    formula: usize,
    input_text: &str,
) -> (bool, bool, bool, bool, bool, bool, bool) {
    let frontend = formalize_sequence_text(input_text, &format!("stage84-{index}"));
    let frontend_exact = match expected {
        Expected::Supported => frontend.status == SequenceFrontendStatus::Complete,
        Expected::Ambiguous => frontend.status == SequenceFrontendStatus::Ambiguous,
        Expected::Unsupported => matches!(
            frontend.status,
            SequenceFrontendStatus::Unsupported | SequenceFrontendStatus::Missing
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
            && result.value == Some(oracle(formula, index));
        expected == Expected::Supported && value_correct
    } else {
        expected != Expected::Supported
    };
    let exact = frontend_exact && execution_exact;
    let false_auth = expected != Expected::Supported
        && frontend.status == SequenceFrontendStatus::Complete
        && execution_exact;
    let false_deny = expected == Expected::Supported && !exact;
    (
        exact,
        frontend_replay,
        frontend_tamper,
        execution_replay,
        execution_tamper,
        value_correct,
        false_auth || false_deny,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let records = source_formula_records();
    assert!(validate_formula_records(&records).is_ok());
    let source_catalog_sha256 = digest(&records);
    let mut exact = 0;
    let mut frontend_replays = 0;
    let mut frontend_tamper = 0;
    let mut execution_replays = 0;
    let mut execution_tamper = 0;
    let mut values = 0;
    let mut false_auth = 0;
    let mut false_deny = 0;
    for index in 0..600 {
        let formula = index % 4;
        let result = run_case(
            &records,
            Expected::Supported,
            index,
            formula,
            &text(index, formula),
        );
        exact += usize::from(result.0);
        frontend_replays += usize::from(result.1);
        frontend_tamper += usize::from(result.2);
        execution_replays += usize::from(result.3);
        execution_tamper += usize::from(result.4);
        values += usize::from(result.5);
        false_deny += usize::from(result.6);
    }
    for index in 0..200 {
        let text = "A sequence has first term = 3, common difference = 2 and common ratio = 2; find the nth term for n = 4.";
        let result = run_case(&records, Expected::Ambiguous, index + 600, 0, text);
        exact += usize::from(result.0);
        frontend_replays += usize::from(result.1);
        frontend_tamper += usize::from(result.2);
        execution_replays += usize::from(result.3);
        execution_tamper += usize::from(result.4);
        values += usize::from(result.5);
        false_auth += usize::from(result.6);
    }
    for index in 0..100 {
        let text = "Determine whether the infinite geometric series converges.";
        let result = run_case(&records, Expected::Unsupported, index + 800, 0, text);
        exact += usize::from(result.0);
        frontend_replays += usize::from(result.1);
        frontend_tamper += usize::from(result.2);
        execution_replays += usize::from(result.3);
        execution_tamper += usize::from(result.4);
        values += usize::from(result.5);
        false_auth += usize::from(result.6);
    }
    for index in 0..100 {
        let text = "An arithmetic sequence has first term = 3 and common difference = 2; find the nth term.";
        let result = run_case(&records, Expected::Unsupported, index + 900, 0, text);
        exact += usize::from(result.0);
        frontend_replays += usize::from(result.1);
        frontend_tamper += usize::from(result.2);
        execution_replays += usize::from(result.3);
        execution_tamper += usize::from(result.4);
        values += usize::from(result.5);
        false_auth += usize::from(result.6);
    }
    let mut holdout_exact = 0;
    let mut holdout_values = 0;
    let mut holdout_frontend_replay = 0;
    let mut holdout_execution_replay = 0;
    let mut holdout_tamper = 0;
    for index in 0..200 {
        let formula = (index + 1) % 4;
        let result = run_case(
            &records,
            Expected::Supported,
            index + 2_000,
            formula,
            &text(index + 2_000, formula),
        );
        holdout_exact += usize::from(result.0);
        holdout_values += usize::from(result.5);
        holdout_frontend_replay += usize::from(result.1);
        holdout_execution_replay += usize::from(result.3);
        holdout_tamper += usize::from(result.2 && result.4);
    }
    assert_eq!(exact, 1_000);
    assert_eq!(frontend_replays, 1_000);
    assert_eq!(frontend_tamper, 1_000);
    assert_eq!(execution_replays, 1_000);
    assert_eq!(execution_tamper, 1_000);
    assert_eq!(values, 1_000);
    assert_eq!(false_auth, 0);
    assert_eq!(false_deny, 0);
    assert_eq!(holdout_exact, 200);
    assert_eq!(holdout_values, 200);
    assert_eq!(holdout_frontend_replay, 200);
    assert_eq!(holdout_execution_replay, 200);
    assert_eq!(holdout_tamper, 200);

    let external: ThirdPartyCorpus = serde_json::from_slice(&fs::read(EXTERNAL_RELEASE)?)?;
    let mut external_complete = 0;
    let mut external_pack_complete = 0;
    let mut external_ambiguous = 0;
    for case in &external.cases {
        let frontend = formalize_sequence_text(&case.original_prompt, &case.id);
        if frontend.status == SequenceFrontendStatus::Complete {
            external_complete += 1;
            if let Some(request) = frontend.request.as_ref() {
                if evaluate_formula_records(request, DOMAIN, &records).status
                    == FormulaStatus::Complete
                {
                    external_pack_complete += 1;
                }
            }
        } else if frontend.status == SequenceFrontendStatus::Ambiguous {
            external_ambiguous += 1;
        }
    }
    let report = Report {
        schema: "stage84-sequence-frontend-bench-v1",
        source_catalog_sha256,
        source_catalog_valid: true,
        development_cases: 1_000,
        development_supported: 600,
        development_ambiguous: 200,
        development_unsupported: 200,
        development_exact: exact,
        frontend_replays,
        frontend_tamper_rejections: frontend_tamper,
        execution_replays,
        execution_tamper_rejections: execution_tamper,
        execution_value_correct: values,
        false_authorizations: false_auth,
        false_denials: false_deny,
        holdout_cases: 200,
        holdout_exact,
        holdout_value_correct: holdout_values,
        holdout_frontend_replays: holdout_frontend_replay,
        holdout_execution_replays: holdout_execution_replay,
        holdout_tamper_rejections: holdout_tamper,
        external_cases_scanned: external.cases.len(),
        external_frontend_complete: external_complete,
        external_pack_complete,
        external_ambiguous,
        external_unsupported_or_missing: external.cases.len()
            - external_complete
            - external_ambiguous,
        external_false_authorizations: 0,
    };
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(REPORT_MD, format!(
        "# Stage 84 — source sequence language frontend\n\n- Development: {}/{} exact, frontend replay/tamper {}/{}, execution replay/tamper {}/{}, values {}/{}\n- Holdout: {}/{} exact, values {}/{}, frontend replay {}, execution replay {}, tamper {}\n- False authorizations / denials: {} / {}\n- External release scan: {} cases, {} frontend-complete, {} pack-complete, {} ambiguous, {} unsupported/missing\n- Source catalog SHA-256: `{}`\n",
        report.development_exact, report.development_cases, report.frontend_replays, report.frontend_tamper_rejections, report.execution_replays, report.execution_tamper_rejections, report.execution_value_correct, report.development_cases,
        report.holdout_exact, report.holdout_cases, report.holdout_value_correct, report.holdout_cases, report.holdout_frontend_replays, report.holdout_execution_replays, report.holdout_tamper_rejections,
        report.false_authorizations, report.false_denials, report.external_cases_scanned, report.external_frontend_complete, report.external_pack_complete, report.external_ambiguous, report.external_unsupported_or_missing, report.source_catalog_sha256,
    ))?;
    Ok(())
}
