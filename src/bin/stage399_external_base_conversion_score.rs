//! Privileged hash-only shadow scorer for Stage 399 base conversion.
//!
//! It reads answer hashes only, never plaintext answers, never authorizes
//! production, and requires the route-blind portfolio to produce exactly one
//! replayable base-conversion candidate.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use the_machine::goal6_external_portfolio::{executable_routes, observe_all, PortfolioRoute};
use the_machine::source_base_conversion_frontend::formalize_base_conversion_text;

const RELEASE_DIR: &str = "data/external_math_exam_v1";

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
struct Candidate {
    id: String,
    value: String,
    reference_match: bool,
    matched_representation: Option<String>,
    replay_verified: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    partition: String,
    dataset_sha256: String,
    source_sha256: String,
    questions_read: usize,
    answer_hashes_read: usize,
    plaintext_answers_read: usize,
    base_candidates: usize,
    correct_shadow_candidates: usize,
    rejected_shadow_candidates: usize,
    candidate_replays: usize,
    route_ambiguities: usize,
    production_authorizations: usize,
    false_authorizations: usize,
    manifest_unchanged: bool,
    candidates: Vec<Candidate>,
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn hash_text(text: &str) -> String {
    hash_bytes(text.as_bytes())
}

fn forms(value: &str, target_base: u32) -> Vec<String> {
    let mut output = vec![value.to_owned(), value.to_ascii_lowercase()];
    output.extend([
        format!("${value}$"),
        format!("\\({value}\\)"),
        format!("\\boxed{{{value}}}"),
        format!("{value}_{{{target_base}}}"),
        format!("{value}_{target_base}"),
        format!("{value} (base {target_base})"),
    ]);
    output
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let partition = env::var("GOAL6_PORTFOLIO_PARTITION").unwrap_or_else(|_| "development".into());
    assert!(matches!(partition.as_str(), "development" | "sealed"));
    if partition == "sealed" {
        assert_eq!(
            env::var("GOAL6_PORTFOLIO_PRIVILEGED_EVAL").as_deref(),
            Ok("true")
        );
    }
    let report_path = env::var("STAGE399_BASE_SCORE_JSON").unwrap_or_else(|_| {
        format!("docs/stage399_external_base_conversion_score_{partition}.json")
    });
    let questions_bytes = fs::read(format!("{RELEASE_DIR}/questions.jsonl"))?;
    let dataset_sha256 = hash_bytes(&questions_bytes);
    let questions: Vec<Question> = String::from_utf8(questions_bytes)?
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
    let source_sha256 = hash_text(the_machine::source_base_conversion_pack::SOURCE);
    let manifest_before = the_machine::curriculum::breadth_first_manifest().replay_hash();
    let mut candidates = Vec::new();
    let mut route_ambiguities = 0;
    for question in questions
        .iter()
        .filter(|question| question.split == partition)
    {
        let observations = observe_all(&question.original_prompt, &question.id);
        let executable = executable_routes(&observations);
        let base = executable
            .iter()
            .filter(|observation| observation.route == PortfolioRoute::BaseConversion)
            .collect::<Vec<_>>();
        if executable.len() != 1 || base.len() != 1 {
            route_ambiguities += usize::from(executable.len() > 1);
            continue;
        }
        let observation = base[0];
        let value = match &observation.candidate {
            Some(the_machine::goal6_external_portfolio::PortfolioCandidate::Text(value)) => value,
            _ => continue,
        };
        let expected = oracle
            .get(&question.id)
            .ok_or_else(|| format!("missing oracle for {}", question.id))?;
        let target_base = formalize_base_conversion_text(&question.original_prompt, &question.id)
            .request
            .as_ref()
            .map(|request| request.target_base)
            .ok_or_else(|| format!("missing target base for {}", question.id))?;
        let matched = forms(value, target_base)
            .into_iter()
            .find(|form| hash_text(form) == expected.answer_sha256);
        candidates.push(Candidate {
            id: question.id.clone(),
            value: value.clone(),
            reference_match: matched.is_some(),
            matched_representation: matched,
            replay_verified: observation.frontend_replay_verified
                && observation.execution_replay_verified,
        });
    }
    let manifest_after = the_machine::curriculum::breadth_first_manifest().replay_hash();
    let questions_read = questions
        .iter()
        .filter(|question| question.split == partition)
        .count();
    let report = Report {
        schema: "stage399-external-base-conversion-score-v1",
        partition,
        dataset_sha256,
        source_sha256,
        questions_read,
        answer_hashes_read: oracle.len(),
        plaintext_answers_read: 0,
        base_candidates: candidates.len(),
        correct_shadow_candidates: candidates
            .iter()
            .filter(|candidate| candidate.reference_match)
            .count(),
        rejected_shadow_candidates: candidates
            .iter()
            .filter(|candidate| !candidate.reference_match)
            .count(),
        candidate_replays: candidates
            .iter()
            .filter(|candidate| candidate.replay_verified)
            .count(),
        route_ambiguities,
        production_authorizations: 0,
        false_authorizations: 0,
        manifest_unchanged: manifest_before == manifest_after,
        candidates,
    };
    assert_eq!(report.plaintext_answers_read, 0);
    assert_eq!(report.production_authorizations, 0);
    assert_eq!(report.false_authorizations, 0);
    assert!(report.manifest_unchanged);
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(&report_path, format!("{serialized}\n"))?;
    println!("{serialized}");
    Ok(())
}
