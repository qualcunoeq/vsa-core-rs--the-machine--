//! Privileged, post-freeze scorer for the source-derived unit route.
//!
//! Only development answer hashes are read.  The sealed partition and all
//! plaintext answers remain inaccessible; this binary never authorizes or
//! mutates a route.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaStatus,
};
use the_machine::source_unit_frontend::{
    formalize_unit_text, replay_verified as frontend_replay, UnitFrontendStatus,
};

const RELEASE_DIR: &str = "data/external_math_exam_v1";
const SOURCE_DOCUMENT: &str = "docs/sources/openstax_unit_conversion_goal6_catalog.txt";
const DOMAIN: &str = "source_catalog_unit_conversion";
const REPORT_JSON: &str = "docs/goal6_external_unit_shadow_score.json";
const REPORT_MD: &str = "docs/goal6_external_unit_shadow_score.md";

#[derive(Debug, Deserialize)]
struct Question {
    id: String,
    original_prompt: String,
    split: String,
}

#[derive(Debug, Deserialize)]
struct Oracle {
    id: String,
    answer_sha256: String,
}

#[derive(Debug, Serialize)]
struct CandidateReceipt {
    id: String,
    candidate_answer_sha256: String,
    reference_match: bool,
    matched_representation: Option<&'static str>,
    replay_verified: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    release_id: &'static str,
    partition: &'static str,
    source_domain: &'static str,
    source_document_sha256: String,
    questions_read: usize,
    answer_hashes_read: usize,
    plaintext_answers_read: usize,
    candidate_cases: usize,
    correct_shadow_candidates: usize,
    incorrect_shadow_candidates_rejected: usize,
    candidate_replays: usize,
    production_authorizations: usize,
    false_authorizations: usize,
    manifest_sha256_before: String,
    manifest_sha256_after: String,
    manifest_unchanged: bool,
    candidates: Vec<CandidateReceipt>,
    report_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn terminating_decimal(value: &the_machine::probability_pack::Rational) -> Option<String> {
    if value.denominator <= 0 {
        return None;
    }
    let mut denominator = value.denominator;
    let mut twos = 0u32;
    let mut fives = 0u32;
    while denominator % 2 == 0 {
        denominator /= 2;
        twos += 1;
    }
    while denominator % 5 == 0 {
        denominator /= 5;
        fives += 1;
    }
    if denominator != 1 {
        return None;
    }
    let places = twos.max(fives);
    let numerator = value
        .numerator
        .checked_mul(2_i128.checked_pow(places - twos)?)?
        .checked_mul(5_i128.checked_pow(places - fives)?)?;
    let negative = numerator < 0;
    let digits = numerator.abs().to_string();
    let rendered = if places == 0 {
        digits
    } else if digits.len() <= places as usize {
        format!("0.{}{}", "0".repeat(places as usize - digits.len()), digits)
    } else {
        let split = digits.len() - places as usize;
        format!("{}.{}", &digits[..split], &digits[split..])
    };
    Some(if negative {
        format!("-{rendered}")
    } else {
        rendered
    })
}

fn candidate_forms(value: &the_machine::probability_pack::Rational) -> Vec<(&'static str, String)> {
    let mut forms = vec![(
        "fraction",
        if value.denominator == 1 {
            value.numerator.to_string()
        } else {
            format!("{}/{}", value.numerator, value.denominator)
        },
    )];
    if let Some(decimal) = terminating_decimal(value) {
        forms.push(("decimal", decimal));
    }
    forms
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_document = fs::read_to_string(SOURCE_DOCUMENT)?;
    let records = extract_formula_records(&source_document).map_err(|errors| errors.join("; "))?;
    let questions: Vec<Question> = fs::read_to_string(format!("{RELEASE_DIR}/questions.jsonl"))?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<Vec<_>, _>>()?;
    let oracle: BTreeMap<String, Oracle> =
        fs::read_to_string(format!("{RELEASE_DIR}/oracle_development.jsonl"))?
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(serde_json::from_str::<Oracle>)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|record| (record.id.clone(), record))
            .collect();
    let manifest_before = breadth_first_manifest().replay_hash();
    let mut candidates = Vec::new();
    for question in questions
        .iter()
        .filter(|question| question.split == "development")
    {
        let frontend = formalize_unit_text(&question.original_prompt, &question.id, &records);
        if frontend.status != UnitFrontendStatus::Complete {
            continue;
        }
        let Some(request) = frontend.request.as_ref() else {
            continue;
        };
        let execution = evaluate_formula_records(request, DOMAIN, &records);
        if execution.status != FormulaStatus::Complete
            || !frontend_replay(&frontend)
            || !execution.replay_verified()
        {
            continue;
        }
        let Some(value) = execution.value.as_ref() else {
            continue;
        };
        let expected = oracle
            .get(&question.id)
            .ok_or_else(|| format!("missing development oracle for {}", question.id))?;
        let forms = candidate_forms(value);
        let mut matched = None;
        for (kind, form) in &forms {
            if digest_bytes(form.as_bytes()) == expected.answer_sha256 {
                matched = Some(*kind);
                break;
            }
        }
        candidates.push(CandidateReceipt {
            id: question.id.clone(),
            candidate_answer_sha256: digest_bytes(forms[0].1.as_bytes()),
            reference_match: matched.is_some(),
            matched_representation: matched,
            replay_verified: frontend_replay(&frontend) && execution.replay_verified(),
        });
    }
    let manifest_after = breadth_first_manifest().replay_hash();
    let mut report = Report {
        schema: "goal6-external-unit-shadow-score-v1",
        release_id: "external-math-exam-v1",
        partition: "development",
        source_domain: DOMAIN,
        source_document_sha256: digest(&source_document),
        questions_read: questions
            .iter()
            .filter(|question| question.split == "development")
            .count(),
        answer_hashes_read: oracle.len(),
        plaintext_answers_read: 0,
        candidate_cases: candidates.len(),
        correct_shadow_candidates: candidates.iter().filter(|c| c.reference_match).count(),
        incorrect_shadow_candidates_rejected: candidates
            .iter()
            .filter(|c| !c.reference_match)
            .count(),
        candidate_replays: candidates.iter().filter(|c| c.replay_verified).count(),
        production_authorizations: 0,
        false_authorizations: 0,
        manifest_sha256_before: manifest_before.clone(),
        manifest_sha256_after: manifest_after.clone(),
        manifest_unchanged: manifest_before == manifest_after,
        candidates,
        report_sha256: String::new(),
    };
    let mut unsigned = serde_json::to_value(&report)?;
    unsigned["report_sha256"] = serde_json::Value::String(String::new());
    report.report_sha256 = digest(&unsigned);
    assert_eq!(report.plaintext_answers_read, 0);
    assert_eq!(report.production_authorizations, 0);
    assert_eq!(report.false_authorizations, 0);
    assert!(report.manifest_unchanged);
    assert_eq!(report.candidate_replays, report.candidate_cases);
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Goal 6 — unit-conversion privileged shadow score\n\n- Development questions read: {}\n- Answer hashes read / plaintext answers read: {} / {}\n- Shadow candidates: {}\n- Correct / rejected shadow candidates: {} / {}\n- Candidate replay: {} / {}\n- Production authorizations / false authorizations: {} / {}\n- Manifest unchanged: {}\n- Source SHA-256: `{}`\n\nThe scorer compares only canonical candidate representations against development hashes; it never reads sealed answers or authorizes production.\n",
            report.questions_read,
            report.answer_hashes_read,
            report.plaintext_answers_read,
            report.candidate_cases,
            report.correct_shadow_candidates,
            report.incorrect_shadow_candidates_rejected,
            report.candidate_replays,
            report.candidate_cases,
            report.production_authorizations,
            report.false_authorizations,
            report.manifest_unchanged,
            report.source_document_sha256,
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
