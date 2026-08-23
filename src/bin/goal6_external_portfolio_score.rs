//! Hash-only scorer for the frozen Goal 6 external portfolio.
//!
//! It reads hash-only answers only in an explicit privileged evaluation mode;
//! ordinary development runs remain development-only and never read sealed
//! questions or answer hashes.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::goal6_external_portfolio::{
    executable_routes, observe_all, PortfolioCandidate, PortfolioRoute,
};

const RELEASE_DIR: &str = "data/external_math_exam_v1";
const PROBE_REPORT: &str = "docs/goal6_external_portfolio_probe.json";
const REPORT_JSON: &str = "docs/goal6_external_portfolio_score.json";
const REPORT_MD: &str = "docs/goal6_external_portfolio_score.md";

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

#[derive(Debug, Deserialize)]
struct ProbeManifest {
    schema: String,
    partition: String,
    report_sha256: String,
    dataset_sha256: String,
    questions_read: usize,
}

#[derive(Debug, Serialize)]
struct CandidateReceipt {
    id: String,
    selected_route: PortfolioRoute,
    candidate_answer_sha256: String,
    reference_match: bool,
    matched_representation: Option<&'static str>,
    replay_verified: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    release_id: &'static str,
    partition: String,
    probe_report_sha256: String,
    dataset_sha256: String,
    questions_read: usize,
    answer_hashes_read: usize,
    plaintext_answers_read: usize,
    sealed_questions_read: usize,
    unique_candidate_cases: usize,
    correct_shadow_candidates: usize,
    incorrect_shadow_candidates_rejected: usize,
    candidate_replays: usize,
    route_ambiguities: usize,
    no_executable_route: usize,
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

fn candidate_forms(candidate: &PortfolioCandidate) -> Vec<(&'static str, String)> {
    match candidate {
        PortfolioCandidate::ExactCount(value) => vec![("integer", value.to_string())],
        PortfolioCandidate::Rational(value) => {
            let mut forms = vec![(
                "fraction",
                if value.denominator == 1 {
                    value.numerator.to_string()
                } else {
                    format!("{}/{}", value.numerator, value.denominator)
                },
            )];
            if value.denominator != 1 {
                forms.push((
                    "latex_fraction",
                    format!("\\frac{{{}}}{{{}}}", value.numerator, value.denominator),
                ));
            }
            if let Some(decimal) = terminating_decimal(value) {
                forms.push(("terminating_decimal", decimal));
            }
            forms
        }
        PortfolioCandidate::Text(value) => vec![("text", value.clone())],
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let partition = env::var("GOAL6_PORTFOLIO_PARTITION").unwrap_or_else(|_| "development".into());
    assert!(matches!(partition.as_str(), "development" | "sealed"));
    if partition == "sealed" {
        assert_eq!(
            env::var("GOAL6_PORTFOLIO_PRIVILEGED_EVAL").as_deref(),
            Ok("true"),
            "sealed scoring requires an explicit privileged-eval flag"
        );
    }
    let probe_report_path =
        env::var("GOAL6_PORTFOLIO_PROBE_REPORT").unwrap_or_else(|_| PROBE_REPORT.into());
    let report_json = env::var("GOAL6_PORTFOLIO_SCORE_JSON").unwrap_or_else(|_| REPORT_JSON.into());
    let report_md = env::var("GOAL6_PORTFOLIO_SCORE_MD").unwrap_or_else(|_| REPORT_MD.into());
    let probe: ProbeManifest = serde_json::from_str(&fs::read_to_string(&probe_report_path)?)?;
    let question_bytes = fs::read(format!("{RELEASE_DIR}/questions.jsonl"))?;
    let dataset_sha256 = digest_bytes(&question_bytes);
    let questions: Vec<Question> = String::from_utf8(question_bytes)?
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
    assert_eq!(probe.schema, "goal6-external-portfolio-probe-v1");
    assert_eq!(probe.partition, partition);
    assert_eq!(
        probe.questions_read,
        if partition == "sealed" { 1000 } else { 3000 }
    );
    assert_eq!(probe.dataset_sha256, dataset_sha256);
    let manifest_sha256_before = breadth_first_manifest().replay_hash();
    let mut candidates = Vec::new();
    let mut route_ambiguities = 0;
    let mut no_executable_route = 0;
    for question in questions
        .iter()
        .filter(|question| question.split == partition)
    {
        let observations = observe_all(&question.original_prompt, &question.id);
        let executable = executable_routes(&observations);
        if executable.len() != 1 {
            if executable.is_empty() {
                no_executable_route += 1;
            } else {
                route_ambiguities += 1;
            }
            continue;
        }
        let selected = executable[0];
        let candidate = selected
            .candidate
            .as_ref()
            .expect("executable route has candidate");
        let expected = oracle
            .get(&question.id)
            .ok_or_else(|| format!("missing {partition} oracle for {}", question.id))?;
        let forms = candidate_forms(candidate);
        let mut matched_representation = None;
        for (kind, form) in &forms {
            if digest_bytes(form.as_bytes()) == expected.answer_sha256 {
                matched_representation = Some(*kind);
                break;
            }
        }
        candidates.push(CandidateReceipt {
            id: question.id.clone(),
            selected_route: selected.route,
            candidate_answer_sha256: digest_bytes(forms[0].1.as_bytes()),
            reference_match: matched_representation.is_some(),
            matched_representation,
            replay_verified: selected.frontend_replay_verified
                && selected.execution_replay_verified,
        });
    }
    let manifest_sha256_after = breadth_first_manifest().replay_hash();
    let mut report = Report {
        schema: "goal6-external-portfolio-score-v1",
        release_id: "external-math-exam-v1",
        partition: partition.clone(),
        probe_report_sha256: probe.report_sha256,
        dataset_sha256,
        questions_read: questions
            .iter()
            .filter(|question| question.split == partition)
            .count(),
        answer_hashes_read: oracle.len(),
        plaintext_answers_read: 0,
        sealed_questions_read: usize::from(partition == "sealed") * oracle.len(),
        unique_candidate_cases: candidates.len(),
        correct_shadow_candidates: candidates
            .iter()
            .filter(|candidate| candidate.reference_match)
            .count(),
        incorrect_shadow_candidates_rejected: candidates
            .iter()
            .filter(|candidate| !candidate.reference_match)
            .count(),
        candidate_replays: candidates
            .iter()
            .filter(|candidate| candidate.replay_verified)
            .count(),
        route_ambiguities,
        no_executable_route,
        production_authorizations: 0,
        false_authorizations: 0,
        manifest_sha256_before: manifest_sha256_before.clone(),
        manifest_sha256_after: manifest_sha256_after.clone(),
        manifest_unchanged: manifest_sha256_before == manifest_sha256_after,
        candidates,
        report_sha256: String::new(),
    };
    let mut unsigned = serde_json::to_value(&report)?;
    unsigned["report_sha256"] = serde_json::Value::String(String::new());
    report.report_sha256 = digest(&unsigned);
    assert_eq!(
        report.questions_read,
        if partition == "sealed" { 1000 } else { 3000 }
    );
    assert_eq!(report.answer_hashes_read, report.questions_read);
    assert_eq!(report.plaintext_answers_read, 0);
    assert_eq!(
        report.sealed_questions_read,
        if partition == "sealed" {
            report.questions_read
        } else {
            0
        }
    );
    assert_eq!(report.production_authorizations, 0);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.candidate_replays, report.unique_candidate_cases);
    assert!(report.manifest_unchanged);
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(&report_json, format!("{serialized}\n"))?;
    fs::write(
        &report_md,
        format!(
            "# Goal 6 — hash-only external portfolio score\n\n\
- {} questions / answer hashes: {} / {}\n\
- Unique candidates: {}\n\
- Correct / rejected candidates: {} / {}\n\
- Candidate replay: {} / {}\n\
- Multiple-route ambiguities / no executable route: {} / {}\n\
- Plaintext answers / sealed questions read: {} / {}\n\
- Production authorizations / false authorizations: {} / {}\n\
- Manifest unchanged: {}\n\n\
The privileged scorer reruns the frozen route-blind portfolio and compares only canonical candidate hashes against the selected partition's oracle hashes.\n",
            partition,
            report.questions_read,
            report.answer_hashes_read,
            report.unique_candidate_cases,
            report.correct_shadow_candidates,
            report.incorrect_shadow_candidates_rejected,
            report.candidate_replays,
            report.unique_candidate_cases,
            report.route_ambiguities,
            report.no_executable_route,
            report.plaintext_answers_read,
            report.sealed_questions_read,
            report.production_authorizations,
            report.false_authorizations,
            report.manifest_unchanged,
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
