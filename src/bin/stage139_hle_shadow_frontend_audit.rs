//! Stage 139: answer-key-blind shadow frontend audit on real HLE text.
//!
//! Four recently validated frontends receive every HLE question.  This audit
//! does not score answers, read answer keys, or alter production routing.  It
//! records only whether a frontend can construct a complete typed request and
//! whether that request would pass its own shadow evaluator.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use the_machine::bounded_arithmetic_functions_frontend::{
    formalize as formalize_arithmetic, replay_verified as arithmetic_replay,
    ArithmeticFrontendStatus,
};
use the_machine::bounded_arithmetic_functions_pack::{
    evaluate as evaluate_arithmetic, ArithmeticFunctionStatus,
};
use the_machine::dirichlet_character_frontend::{
    formalize as formalize_character, replay_verified as character_replay, CharacterFrontendStatus,
};
use the_machine::dirichlet_character_pack::{evaluate as evaluate_character, CharacterStatus};
use the_machine::number_theory_frontend::{
    formalize_number_theory_text, replay_verified as number_replay, NumberTheoryFrontendStatus,
};
use the_machine::number_theory_pack::{evaluate_number_theory, NumberTheoryStatus};
use the_machine::simplicial_homology_frontend::{
    formalize as formalize_homology, FrontendStatus as HomologyFrontendStatus,
};
use the_machine::simplicial_homology_pack::{evaluate as evaluate_homology, HomologyStatus};

const DATASET: &str = "data/hle.jsonl";
const SUMMARY: &str = "docs/stage139_hle_shadow_frontend_audit.json";
const TRACE: &str = "docs/stage139_hle_shadow_frontend_audit.trace.jsonl";
const SUMMARY_REPAIRED: &str = "docs/stage142_hle_shadow_frontend_repair.json";
const TRACE_REPAIRED: &str = "docs/stage142_hle_shadow_frontend_repair.trace.jsonl";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Family {
    ArithmeticFunctions,
    NumberTheory,
    FiniteCharacter,
    SimplicialHomology,
}

#[derive(Debug, Clone, Copy)]
struct Observation {
    complete: bool,
    authorized: bool,
    replay_verified: bool,
    tamper_rejected: bool,
}

#[derive(Debug, Serialize)]
struct Record {
    id: Option<String>,
    category: String,
    question_sha256: String,
    frontend_statuses: BTreeMap<String, String>,
    complete_candidates: Vec<Family>,
    unique_candidate: Option<Family>,
    shadow_terminal: String,
    candidate_shadow_authorized: bool,
    frontend_replay_verified: bool,
    frontend_tamper_rejected: bool,
    candidate_replay_verified: bool,
    candidate_tamper_rejected: bool,
}

#[derive(Debug, Serialize)]
struct Summary {
    schema: &'static str,
    source: &'static str,
    dataset_sha256: String,
    trace_sha256: String,
    cases: usize,
    frontend_invocations: usize,
    no_complete_candidate: usize,
    unique_complete_candidate: usize,
    multiple_complete_candidates: usize,
    candidate_shadow_authorizations: usize,
    frontend_replay_verified: usize,
    frontend_tamper_rejected: usize,
    candidate_replay_verified: usize,
    candidate_tamper_rejected: usize,
    production_authorizations: usize,
    registry_mutated: bool,
    trace_path: &'static str,
}

fn digest<T: serde::Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn observe_arithmetic(text: &str, id: &str) -> (Observation, String) {
    let frontend = formalize_arithmetic(text, id);
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let frontend_replay_verified = arithmetic_replay(&frontend);
    let frontend_tamper_rejected = !arithmetic_replay(&frontend_tampered);
    let Some(request) = frontend.request.as_ref() else {
        return (
            Observation {
                complete: frontend.status == ArithmeticFrontendStatus::Complete,
                authorized: false,
                replay_verified: frontend_replay_verified,
                tamper_rejected: frontend_tamper_rejected,
            },
            format!("{:?}", frontend.status),
        );
    };
    let result = evaluate_arithmetic(request);
    let mut result_tampered = result.clone();
    result_tampered.replay_hash.push('x');
    (
        Observation {
            complete: frontend.status == ArithmeticFrontendStatus::Complete,
            authorized: result.status == ArithmeticFunctionStatus::Complete
                && result.replay_verified(),
            replay_verified: frontend_replay_verified && result.replay_verified(),
            tamper_rejected: frontend_tamper_rejected && !result_tampered.replay_verified(),
        },
        format!("{:?}", frontend.status),
    )
}

fn observe_number(text: &str, id: &str) -> (Observation, String) {
    let frontend = formalize_number_theory_text(text, id);
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let frontend_replay_verified = number_replay(&frontend);
    let frontend_tamper_rejected = !number_replay(&frontend_tampered);
    let Some(request) = frontend.request.as_ref() else {
        return (
            Observation {
                complete: frontend.status == NumberTheoryFrontendStatus::Complete,
                authorized: false,
                replay_verified: frontend_replay_verified,
                tamper_rejected: frontend_tamper_rejected,
            },
            format!("{:?}", frontend.status),
        );
    };
    let result = evaluate_number_theory(request);
    let mut result_tampered = result.clone();
    result_tampered.replay_hash.push('x');
    (
        Observation {
            complete: frontend.status == NumberTheoryFrontendStatus::Complete,
            authorized: result.status == NumberTheoryStatus::Complete
                && result.artifact.is_some()
                && result.replay_verified(),
            replay_verified: frontend_replay_verified && result.replay_verified(),
            tamper_rejected: frontend_tamper_rejected && !result_tampered.replay_verified(),
        },
        format!("{:?}", frontend.status),
    )
}

fn observe_character(text: &str, id: &str) -> (Observation, String) {
    let frontend = formalize_character(text, id);
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let frontend_replay_verified = character_replay(&frontend);
    let frontend_tamper_rejected = !character_replay(&frontend_tampered);
    let Some(request) = frontend.request.as_ref() else {
        return (
            Observation {
                complete: frontend.status == CharacterFrontendStatus::Complete,
                authorized: false,
                replay_verified: frontend_replay_verified,
                tamper_rejected: frontend_tamper_rejected,
            },
            format!("{:?}", frontend.status),
        );
    };
    let result = evaluate_character(request);
    let mut result_tampered = result.clone();
    result_tampered.replay_hash.push('x');
    (
        Observation {
            complete: frontend.status == CharacterFrontendStatus::Complete,
            authorized: result.status == CharacterStatus::Complete && result.authorized(),
            replay_verified: frontend_replay_verified && result.replay_verified(),
            tamper_rejected: frontend_tamper_rejected && !result_tampered.replay_verified(),
        },
        format!("{:?}", frontend.status),
    )
}

fn observe_homology(text: &str) -> (Observation, String) {
    let frontend = formalize_homology(text);
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let frontend_replay_verified = frontend.replay_verified();
    let frontend_tamper_rejected = !frontend_tampered.replay_verified();
    let Some(request) = frontend.request.as_ref() else {
        return (
            Observation {
                complete: frontend.status == HomologyFrontendStatus::Complete,
                authorized: false,
                replay_verified: frontend_replay_verified,
                tamper_rejected: frontend_tamper_rejected,
            },
            format!("{:?}", frontend.status),
        );
    };
    let result = evaluate_homology(request);
    let mut result_tampered = result.clone();
    result_tampered.replay_hash.push('x');
    (
        Observation {
            complete: frontend.status == HomologyFrontendStatus::Complete,
            authorized: result.status == HomologyStatus::Complete && result.authorized(),
            replay_verified: frontend_replay_verified && result.replay_verified(),
            tamper_rejected: frontend_tamper_rejected && !result_tampered.replay_verified(),
        },
        format!("{:?}", frontend.status),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repaired = std::env::var_os("STAGE142_REPAIRED").is_some();
    let dataset = fs::read(DATASET)?;
    let trace_path = if repaired { TRACE_REPAIRED } else { TRACE };
    let summary_path = if repaired { SUMMARY_REPAIRED } else { SUMMARY };
    let mut trace_file = File::create(trace_path)?;
    let mut cases = 0usize;
    let mut no_complete_candidate = 0usize;
    let mut unique_complete_candidate = 0usize;
    let mut multiple_complete_candidates = 0usize;
    let mut candidate_shadow_authorizations = 0usize;
    let mut frontend_replay_verified = 0usize;
    let mut frontend_tamper_rejected = 0usize;
    let mut candidate_replay_verified = 0usize;
    let mut candidate_tamper_rejected = 0usize;
    for line in BufReader::new(File::open(DATASET)?).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: Value = serde_json::from_str(&line)?;
        let question = entry.get("question").and_then(Value::as_str).unwrap_or("");
        let id = entry.get("id").and_then(Value::as_str).unwrap_or("");
        let (arithmetic, arithmetic_status) = observe_arithmetic(question, id);
        let (number, number_status) = observe_number(question, id);
        let (character, character_status) = observe_character(question, id);
        let (homology, homology_status) = observe_homology(question);
        let observations = [
            (Family::ArithmeticFunctions, arithmetic, arithmetic_status),
            (Family::NumberTheory, number, number_status),
            (Family::FiniteCharacter, character, character_status),
            (Family::SimplicialHomology, homology, homology_status),
        ];
        let complete_candidates: Vec<Family> = observations
            .iter()
            .filter(|(_, observation, _)| observation.complete)
            .map(|(family, _, _)| *family)
            .collect();
        let unique_candidate = (complete_candidates.len() == 1).then(|| complete_candidates[0]);
        if complete_candidates.is_empty() {
            no_complete_candidate += 1;
        } else if complete_candidates.len() == 1 {
            unique_complete_candidate += 1;
        } else {
            multiple_complete_candidates += 1;
        }
        let candidate = unique_candidate
            .and_then(|family| {
                observations
                    .iter()
                    .find(|(candidate_family, _, _)| *candidate_family == family)
            })
            .map(|(_, observation, _)| *observation);
        let candidate_shadow_authorized =
            candidate.is_some_and(|observation| observation.authorized);
        if candidate_shadow_authorized {
            candidate_shadow_authorizations += 1;
        }
        let frontend_replay = observations
            .iter()
            .all(|(_, observation, _)| observation.replay_verified);
        let frontend_tamper = observations
            .iter()
            .all(|(_, observation, _)| observation.tamper_rejected);
        let candidate_replay = candidate.is_none_or(|observation| observation.replay_verified);
        let candidate_tamper = candidate.is_none_or(|observation| observation.tamper_rejected);
        frontend_replay_verified += usize::from(frontend_replay);
        frontend_tamper_rejected += usize::from(frontend_tamper);
        candidate_replay_verified += usize::from(candidate.is_some() && candidate_replay);
        candidate_tamper_rejected += usize::from(candidate.is_some() && candidate_tamper);
        let shadow_terminal = if complete_candidates.len() > 1 {
            "multiple_complete_candidates"
        } else if candidate_shadow_authorized {
            "unique_shadow_authorized_candidate"
        } else if unique_candidate.is_some() {
            "unique_candidate_execution_refused"
        } else {
            "no_complete_candidate"
        };
        let statuses = observations
            .iter()
            .map(|(family, _, status)| (format!("{:?}", family), status.clone()))
            .collect();
        let record = Record {
            id: entry.get("id").and_then(Value::as_str).map(str::to_owned),
            category: entry
                .get("category")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            question_sha256: digest_bytes(question.as_bytes()),
            frontend_statuses: statuses,
            complete_candidates,
            unique_candidate,
            shadow_terminal: shadow_terminal.into(),
            candidate_shadow_authorized,
            frontend_replay_verified: frontend_replay,
            frontend_tamper_rejected: frontend_tamper,
            candidate_replay_verified: candidate.is_some() && candidate_replay,
            candidate_tamper_rejected: candidate.is_some() && candidate_tamper,
        };
        serde_json::to_writer(&mut trace_file, &record)?;
        trace_file.write_all(b"\n")?;
        cases += 1;
    }
    trace_file.flush()?;
    let trace = fs::read(trace_path)?;
    let summary = Summary {
        schema: if repaired {
            "stage142-hle-shadow-frontend-repair-v1"
        } else {
            "stage139-hle-shadow-frontend-audit-v1"
        },
        source: "answer-key-blind HLE text offered to four validated shadow frontends",
        dataset_sha256: digest_bytes(&dataset),
        trace_sha256: digest_bytes(&trace),
        cases,
        frontend_invocations: cases * 4,
        no_complete_candidate,
        unique_complete_candidate,
        multiple_complete_candidates,
        candidate_shadow_authorizations,
        frontend_replay_verified,
        frontend_tamper_rejected,
        candidate_replay_verified,
        candidate_tamper_rejected,
        production_authorizations: 0,
        registry_mutated: false,
        trace_path,
    };
    assert_eq!(summary.cases, 2500);
    assert_eq!(summary.frontend_invocations, 10_000);
    assert_eq!(summary.frontend_replay_verified, summary.cases);
    assert_eq!(summary.frontend_tamper_rejected, summary.cases);
    assert_eq!(summary.production_authorizations, 0);
    assert!(!summary.trace_sha256.is_empty());
    fs::write(summary_path, serde_json::to_vec_pretty(&summary)?)?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}
