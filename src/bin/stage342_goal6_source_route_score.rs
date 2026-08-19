//! Stage 342: privileged hash-only scoring of the route selected by Stage 341.
//!
//! Normal source selection and transfer remain answer-key blind. This binary
//! is an explicit evaluation boundary: it reads only canonical answer hashes,
//! never plaintext answers, and never mutates production routing.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use the_machine::source_formula_frontend::formalize_source_formula_text;
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaStatus,
};

const SELECTOR_REPORT: &str = "docs/stage341_goal6_source_route_selection.json";
const QUESTIONS_PATH: &str = "data/external_math_exam_v1/questions.jsonl";
const RELEASE_DIR: &str = "data/external_math_exam_v1";
const DEFAULT_JSON: &str = "docs/stage342_goal6_source_route_score.json";
const DEFAULT_MD: &str = "docs/stage342_goal6_source_route_score.md";

#[derive(Debug, Deserialize)]
struct SelectorReport {
    report_sha256: String,
    dataset_sha256: String,
    selected_route: String,
    selected_source_path: String,
    selected_source_sha256: String,
    route_observations: Vec<SelectedRouteObservation>,
    answer_keys_read: usize,
    plaintext_answers_read: usize,
    production_mutations: usize,
    manifest_unchanged: bool,
}

#[derive(Debug, Deserialize)]
struct SelectedRouteObservation {
    route: String,
    development_candidate_hashes: Vec<String>,
    sealed_candidate_hashes: Vec<String>,
}

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
    candidate_hash: String,
    reference_match: bool,
    matched_representation: Option<&'static str>,
    replay_verified: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    selector_report_sha256: String,
    partition: String,
    selected_route: String,
    selected_source_path: String,
    selected_source_sha256: String,
    dataset_sha256: String,
    questions_read: usize,
    answer_hashes_read: usize,
    plaintext_answers_read: usize,
    candidate_count: usize,
    correct_candidates: usize,
    incorrect_candidates_rejected: usize,
    candidate_replays: usize,
    no_candidate: usize,
    production_authorizations: usize,
    false_authorizations: usize,
    manifest_unchanged: bool,
    candidates: Vec<CandidateReceipt>,
    report_sha256: String,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
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
        forms.push(("terminating_decimal", decimal));
    }
    forms
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        env::var("GOAL6_SOURCE_ROUTE_PRIVILEGED_EVAL").as_deref(),
        Ok("true"),
        "hash-only route scoring requires explicit privileged evaluation"
    );
    let partition = env::var("GOAL6_PORTFOLIO_PARTITION").unwrap_or_else(|_| "development".into());
    assert!(matches!(partition.as_str(), "development" | "sealed"));
    let selector: SelectorReport = serde_json::from_slice(&fs::read(SELECTOR_REPORT)?)?;
    let report_json =
        env::var("GOAL6_SOURCE_ROUTE_SCORE_JSON").unwrap_or_else(|_| DEFAULT_JSON.into());
    let report_md = env::var("GOAL6_SOURCE_ROUTE_SCORE_MD").unwrap_or_else(|_| DEFAULT_MD.into());
    assert_eq!(selector.answer_keys_read, 0);
    assert_eq!(selector.plaintext_answers_read, 0);
    assert_eq!(selector.production_mutations, 0);
    assert!(selector.manifest_unchanged);
    let source_bytes = fs::read(&selector.selected_source_path)?;
    assert_eq!(digest_bytes(&source_bytes), selector.selected_source_sha256);
    let records = extract_formula_records(std::str::from_utf8(&source_bytes)?)
        .map_err(|errors| format!("source extraction failed: {errors:?}"))?;
    let question_bytes = fs::read(QUESTIONS_PATH)?;
    assert_eq!(digest_bytes(&question_bytes), selector.dataset_sha256);
    let questions: Vec<Question> = std::str::from_utf8(&question_bytes)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<Vec<_>, _>>()?;
    let oracle: BTreeMap<String, Oracle> =
        fs::read_to_string(format!("{RELEASE_DIR}/oracle_{partition}.jsonl"))?
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(serde_json::from_str::<Oracle>)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|record| (record.id.clone(), record))
            .collect();
    let selected_observation = selector
        .route_observations
        .iter()
        .find(|observation| observation.route == selector.selected_route)
        .ok_or("selector report lacks the selected route observation")?;
    let expected_hashes = if partition == "development" {
        &selected_observation.development_candidate_hashes
    } else {
        &selected_observation.sealed_candidate_hashes
    };
    let domain = {
        let mut value = String::from("goal6_source_selected_");
        for character in selector.selected_route.chars() {
            if character.is_ascii_alphanumeric() {
                value.push(character.to_ascii_lowercase());
            } else {
                value.push('_');
            }
        }
        value
    };
    let mut candidates = Vec::new();
    let mut no_candidate = 0;
    for question in questions
        .iter()
        .filter(|question| question.split == partition)
    {
        let frontend = formalize_source_formula_text(&question.original_prompt, &domain, &records);
        let Some(request) = frontend.request.as_ref() else {
            no_candidate += 1;
            continue;
        };
        let execution = evaluate_formula_records(request, &domain, &records);
        if execution.status != FormulaStatus::Complete || !execution.replay_verified() {
            no_candidate += 1;
            continue;
        }
        let mut tampered = execution.clone();
        tampered.replay_hash.push('x');
        if tampered.replay_verified() {
            no_candidate += 1;
            continue;
        }
        let candidate_hash = digest(&execution.value);
        assert!(
            expected_hashes.contains(&candidate_hash),
            "selector candidate set differs from scored route"
        );
        let expected = oracle
            .get(&question.id)
            .ok_or_else(|| format!("missing oracle for {}", question.id))?;
        let matched_representation = execution.value.as_ref().and_then(|value| {
            candidate_forms(value).into_iter().find_map(|(kind, form)| {
                (digest_bytes(form.as_bytes()) == expected.answer_sha256).then_some(kind)
            })
        });
        candidates.push(CandidateReceipt {
            id: question.id.clone(),
            candidate_hash,
            reference_match: matched_representation.is_some(),
            matched_representation,
            replay_verified: execution.replay_verified(),
        });
    }
    assert_eq!(candidates.len(), expected_hashes.len());
    let mut report = Report {
        schema: "stage342-goal6-source-route-score-v1",
        selector_report_sha256: selector.report_sha256,
        partition: partition.clone(),
        selected_route: selector.selected_route,
        selected_source_path: selector.selected_source_path,
        selected_source_sha256: selector.selected_source_sha256,
        dataset_sha256: selector.dataset_sha256,
        questions_read: questions
            .iter()
            .filter(|question| question.split == partition)
            .count(),
        answer_hashes_read: oracle.len(),
        plaintext_answers_read: 0,
        candidate_count: candidates.len(),
        correct_candidates: candidates
            .iter()
            .filter(|candidate| candidate.reference_match)
            .count(),
        incorrect_candidates_rejected: candidates
            .iter()
            .filter(|candidate| !candidate.reference_match)
            .count(),
        candidate_replays: candidates
            .iter()
            .filter(|candidate| candidate.replay_verified)
            .count(),
        no_candidate,
        production_authorizations: 0,
        false_authorizations: 0,
        manifest_unchanged: selector.manifest_unchanged,
        candidates,
        report_sha256: String::new(),
    };
    let mut unsigned = serde_json::to_value(&report)?;
    unsigned["report_sha256"] = serde_json::Value::String(String::new());
    report.report_sha256 = digest(&unsigned);
    fs::write(
        &report_json,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        &report_md,
        format!(
            "# Stage 342 — source-selected route score\n\n- Partition / questions / answer hashes: {} / {} / {}\n- Selected route: `{}`\n- Candidates / correct / rejected: {} / {} / {}\n- Candidate replay: {}/{}\n- Plaintext answers / production authorizations / false authorizations: {} / {} / {}\n- No candidate: {}\n\nThis is an explicit privileged hash-only score. Normal source selection remains answer-key blind; no candidate authorizes production routing.\n",
            report.partition,
            report.questions_read,
            report.answer_hashes_read,
            report.selected_route,
            report.candidate_count,
            report.correct_candidates,
            report.incorrect_candidates_rejected,
            report.candidate_replays,
            report.candidate_count,
            report.plaintext_answers_read,
            report.production_authorizations,
            report.false_authorizations,
            report.no_candidate,
        ),
    )?;
    println!(
        "Stage 342 — partition={} candidates={} correct={} replay={}/{} false_auth=0",
        report.partition,
        report.candidate_count,
        report.correct_candidates,
        report.candidate_replays,
        report.candidate_count,
    );
    Ok(())
}
