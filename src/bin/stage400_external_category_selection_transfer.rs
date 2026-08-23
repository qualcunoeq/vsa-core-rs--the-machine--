//! Answer-key-blind transfer probe for the category-selection route.
//!
//! It reads only the frozen external questions and records whether the new
//! route reaches a unique executable boundary. It never reads answer keys or
//! mutates production state.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::goal6_external_portfolio::{executable_routes, observe_all, PortfolioRoute};

const RELEASE_DIR: &str = "data/external_math_exam_v1";

#[derive(Debug, Deserialize)]
struct Question {
    id: String,
    original_prompt: String,
    split: String,
}

#[derive(Debug, Serialize)]
struct Candidate {
    id: String,
    prompt_sha256: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    partition: String,
    dataset_sha256: String,
    questions_read: usize,
    answer_keys_read: usize,
    route_invocations: usize,
    category_frontend_status_counts: BTreeMap<String, usize>,
    category_executable_candidates: usize,
    category_candidates: Vec<Candidate>,
    multiple_route_ambiguities: usize,
    production_authorizations: usize,
    false_authorizations: usize,
    manifest_unchanged: bool,
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn increment(map: &mut BTreeMap<String, usize>, value: impl Into<String>) {
    *map.entry(value.into()).or_default() += 1;
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
    let json_path = env::var("STAGE400_CATEGORY_TRANSFER_JSON").unwrap_or_else(|_| {
        format!("docs/stage400_external_category_selection_transfer_{partition}.json")
    });
    let md_path = env::var("STAGE400_CATEGORY_TRANSFER_MD").unwrap_or_else(|_| {
        format!("docs/stage400_external_category_selection_transfer_{partition}.md")
    });
    let bytes = fs::read(format!("{RELEASE_DIR}/questions.jsonl"))?;
    let dataset_sha256 = hash_bytes(&bytes);
    let all_questions: Vec<Question> = String::from_utf8(bytes)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<Vec<_>, _>>()?;
    let questions: Vec<Question> = all_questions
        .into_iter()
        .filter(|question| question.split == partition)
        .collect();
    let manifest_before = breadth_first_manifest().replay_hash();
    let route_count = observe_all("", "route-count").len();
    let mut statuses = BTreeMap::new();
    let mut candidates = Vec::new();
    let mut multiple_route_ambiguities = 0;
    for question in &questions {
        let observations = observe_all(&question.original_prompt, &question.id);
        let category = observations
            .iter()
            .find(|observation| observation.route == PortfolioRoute::CategorySelection)
            .expect("portfolio must expose category selection route");
        increment(&mut statuses, category.frontend_status.clone());
        let executable = executable_routes(&observations);
        if executable.len() > 1 {
            multiple_route_ambiguities += 1;
        }
        if executable.len() == 1 && executable[0].route == PortfolioRoute::CategorySelection {
            candidates.push(Candidate {
                id: question.id.clone(),
                prompt_sha256: hash_bytes(question.original_prompt.as_bytes()),
            });
        }
    }
    let manifest_after = breadth_first_manifest().replay_hash();
    let report = Report {
        schema: "stage400-external-category-selection-transfer-v1",
        partition,
        dataset_sha256,
        questions_read: questions.len(),
        answer_keys_read: 0,
        route_invocations: questions.len() * route_count,
        category_frontend_status_counts: statuses,
        category_executable_candidates: candidates.len(),
        category_candidates: candidates,
        multiple_route_ambiguities,
        production_authorizations: 0,
        false_authorizations: 0,
        manifest_unchanged: manifest_before == manifest_after,
    };
    assert_eq!(report.answer_keys_read, 0);
    assert_eq!(report.production_authorizations, 0);
    assert_eq!(report.false_authorizations, 0);
    assert!(report.manifest_unchanged);
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(&json_path, format!("{serialized}\n"))?;
    fs::write(
        &md_path,
        format!(
            "# Stage 400 — category-selection external transfer ({})\n\n- Questions: {}\n- Route invocations: {}\n- Category frontend statuses: {:?}\n- Unique category candidates: {}\n- Multiple-route ambiguities: {}\n- Answer keys read: 0\n- Production authorizations / false authorizations: 0 / 0\n- Manifest unchanged: {}\n\nThis is an answer-key-blind shadow transfer. The route requires explicit equal category sizes; related prompts without that premise remain non-executable.\n",
            report.partition,
            report.questions_read,
            report.route_invocations,
            report.category_frontend_status_counts,
            report.category_executable_candidates,
            report.multiple_route_ambiguities,
            report.manifest_unchanged,
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
