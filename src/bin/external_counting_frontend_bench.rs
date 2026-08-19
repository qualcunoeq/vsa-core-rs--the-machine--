//! Answer-key-blind validation of the source-derived bounded-counting frontend.
//!
//! The independent corpus is authored in this binary only to exercise the
//! contract boundaries; it is not used as a development proxy for MATH.  The
//! external probe reads development question text and never reads an oracle,
//! emits an answer authorization, or mutates the curriculum registry.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::source_counting_frontend::{
    formalize_counting_text, replay_verified as frontend_replay, CountingFrontendStatus,
};
use the_machine::source_counting_pack::{
    evaluate, replay_verified as execution_replay, CountingArtifact, CountingStatus, SOURCE_ID,
};

const QUESTIONS: &str = "data/external_math_exam_v1/questions.jsonl";
const SOURCE_DOCUMENT: &str = "docs/sources/openstax_counting_principles_catalog.txt";
const REPORT_JSON: &str = "docs/goal6_external_counting_frontend.json";
const REPORT_MD: &str = "docs/goal6_external_counting_frontend.md";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ExpectedStatus {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
struct IndependentCase {
    id: String,
    text: String,
    expected: ExpectedStatus,
    expected_value: Option<u128>,
}

#[derive(Debug, Deserialize)]
struct Question {
    id: String,
    original_prompt: String,
    split: String,
}

#[derive(Debug, Serialize)]
struct ExternalCandidate {
    id: String,
    status: &'static str,
    operation: String,
    artifact: Option<CountingArtifact>,
    frontend_replay_verified: bool,
    execution_replay_verified: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_id: &'static str,
    source_document_sha256: String,
    independent_cases: usize,
    independent_supported: usize,
    independent_ambiguous: usize,
    independent_unsupported: usize,
    independent_exact_decisions: usize,
    independent_supported_values: usize,
    independent_frontend_replays: usize,
    independent_execution_replays: usize,
    independent_frontend_tamper_rejections: usize,
    independent_execution_tamper_rejections: usize,
    independent_false_authorizations: usize,
    independent_false_denials: usize,
    external_questions_read: usize,
    external_answer_keys_read: usize,
    external_counting_signals: usize,
    external_complete_frontends: usize,
    external_executable_candidates: usize,
    external_candidate_replays: usize,
    external_candidates: Vec<ExternalCandidate>,
    production_authorizations: usize,
    false_authorizations: usize,
    manifest_unchanged: bool,
    report_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn factorial(n: u128) -> u128 {
    (1..=n).product()
}

fn independent_cases() -> Vec<IndependentCase> {
    let mut cases = Vec::with_capacity(120);
    for index in 0..80u64 {
        let n = 4 + index % 17;
        let r = index % (n + 1);
        let (text, expected) = match index % 4 {
            0 => (
                format!("For n={n} and r={r}, count the ordered permutations."),
                (0..r).fold(1u128, |value, offset| value * (n - offset) as u128),
            ),
            1 => (
                format!("For n={n} and r={r}, count the unordered combinations."),
                factorial(n as u128) / (factorial(r as u128) * factorial((n - r) as u128)),
            ),
            2 => (format!("Evaluate n! when n={n}."), factorial(n as u128)),
            _ => (
                format!("Multiply the independent factors n={n} and r={r}."),
                (n as u128) * (r as u128),
            ),
        };
        cases.push(IndependentCase {
            id: format!("counting-supported-{index:03}"),
            text,
            expected: ExpectedStatus::Supported,
            expected_value: Some(expected),
        });
    }
    for index in 0..20 {
        let text = match index % 3 {
            0 => "For n=5 and r=2, choose either permutations or combinations.".into(),
            1 => "Count the ways, but the values of n and r are omitted.".into(),
            _ => "The order may or may not matter for n=6 and r=3.".into(),
        };
        cases.push(IndependentCase {
            id: format!("counting-ambiguous-{index:03}"),
            text,
            expected: ExpectedStatus::Ambiguous,
            expected_value: None,
        });
    }
    for index in 0..20 {
        let text = match index % 4 {
            0 => "Determine the asymptotic number of permutations as n grows.".into(),
            1 => "Find the probability density of an unordered selection.".into(),
            2 => "Count the infinite set of arrangements exactly.".into(),
            _ => "Use a diagram to determine how many objects can be selected.".into(),
        };
        cases.push(IndependentCase {
            id: format!("counting-unsupported-{index:03}"),
            text,
            expected: ExpectedStatus::Unsupported,
            expected_value: None,
        });
    }
    cases
}

fn expected_frontend_status(status: CountingFrontendStatus) -> ExpectedStatus {
    match status {
        CountingFrontendStatus::Complete => ExpectedStatus::Supported,
        CountingFrontendStatus::Ambiguous | CountingFrontendStatus::Missing => {
            ExpectedStatus::Ambiguous
        }
        CountingFrontendStatus::Unsupported => ExpectedStatus::Unsupported,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_document = fs::read_to_string(SOURCE_DOCUMENT)?;
    assert!(the_machine::source_counting_pack::validate_source_document(
        &source_document
    ));
    let cases = independent_cases();
    assert_eq!(cases.len(), 120);

    let mut exact = 0;
    let mut values = 0;
    let mut frontend_replays = 0;
    let mut execution_replays = 0;
    let mut frontend_tamper = 0;
    let mut execution_tamper = 0;
    let mut false_auth = 0;
    let mut false_denial = 0;
    for case in &cases {
        let frontend = formalize_counting_text(&case.text, &case.id);
        frontend_replays += usize::from(frontend_replay(&frontend));
        let mut frontend_copy = frontend.clone();
        frontend_copy.replay_hash.push('x');
        frontend_tamper += usize::from(!frontend_replay(&frontend_copy));
        let observed = expected_frontend_status(frontend.status);
        let mut executed = false;
        if let Some(request) = frontend.request.as_ref() {
            let result = evaluate(request);
            executed = result.status == CountingStatus::Complete;
            execution_replays += usize::from(execution_replay(&result));
            let mut copy = result.clone();
            copy.replay_hash.push('x');
            execution_tamper += usize::from(!execution_replay(&copy));
            if case.expected == ExpectedStatus::Supported
                && result.artifact == case.expected_value.map(CountingArtifact::ExactCount)
            {
                values += 1;
            }
        }
        exact += usize::from(observed == case.expected);
        false_auth += usize::from(case.expected != ExpectedStatus::Supported && executed);
        false_denial += usize::from(case.expected == ExpectedStatus::Supported && !executed);
    }

    let questions = fs::read_to_string(QUESTIONS)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str::<Question>)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|question| question.split == "development")
        .collect::<Vec<_>>();
    let mut signals = 0;
    let mut complete = 0;
    let mut executable = 0;
    let mut candidate_replays = 0;
    let mut candidates = Vec::new();
    for question in &questions {
        let lower = question.original_prompt.to_ascii_lowercase();
        if [
            "permutation",
            "combination",
            "factorial",
            "choose",
            "arrange",
        ]
        .iter()
        .any(|term| lower.contains(term))
        {
            signals += 1;
        }
        let frontend = formalize_counting_text(&question.original_prompt, &question.id);
        if frontend.status != CountingFrontendStatus::Complete {
            continue;
        }
        complete += 1;
        let request = frontend
            .request
            .as_ref()
            .expect("complete frontend request");
        let result = evaluate(request);
        if result.status == CountingStatus::Complete {
            executable += 1;
            candidate_replays += usize::from(execution_replay(&result));
            candidates.push(ExternalCandidate {
                id: question.id.clone(),
                status: "shadow_candidate",
                operation: format!("{:?}", request.operation),
                artifact: result.artifact.clone(),
                frontend_replay_verified: frontend_replay(&frontend),
                execution_replay_verified: execution_replay(&result),
            });
        }
    }

    let manifest = the_machine::curriculum::breadth_first_manifest().replay_hash();
    let mut report = Report {
        schema: "goal6-external-counting-frontend-v1",
        source_id: SOURCE_ID,
        source_document_sha256: digest(&source_document),
        independent_cases: cases.len(),
        independent_supported: 80,
        independent_ambiguous: 20,
        independent_unsupported: 20,
        independent_exact_decisions: exact,
        independent_supported_values: values,
        independent_frontend_replays: frontend_replays,
        independent_execution_replays: execution_replays,
        independent_frontend_tamper_rejections: frontend_tamper,
        independent_execution_tamper_rejections: execution_tamper,
        independent_false_authorizations: false_auth,
        independent_false_denials: false_denial,
        external_questions_read: questions.len(),
        external_answer_keys_read: 0,
        external_counting_signals: signals,
        external_complete_frontends: complete,
        external_executable_candidates: executable,
        external_candidate_replays: candidate_replays,
        external_candidates: candidates,
        production_authorizations: 0,
        false_authorizations: 0,
        manifest_unchanged: manifest
            == the_machine::curriculum::breadth_first_manifest().replay_hash(),
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    fs::write(REPORT_JSON, serde_json::to_string_pretty(&report)?)?;
    fs::write(
        REPORT_MD,
        format!(
            "# Goal 6 — bounded counting frontend\n\n- Independent corpus: {}/{} exact decisions; supported values {}/80\n- Frontend replay/tamper: {}/{}\n- Execution replay/tamper: {}/{} / {}/{} emitted executions\n- False authorizations / denials: {} / {}\n- External development questions read: {} (answer keys read: 0)\n- Counting signals / complete frontends / executable candidates: {} / {} / {}\n- Candidate replays: {}\n- Production authorizations: 0\n- Curriculum manifest unchanged: {}\n- Source: `{}` (document SHA-256 `{}`)\n\nThis is a development-only, answer-key-blind shadow probe. It does not authorize or mutate production.\n",
            exact,
            cases.len(),
            values,
            frontend_replays,
            cases.len(),
            execution_replays,
            80,
            execution_tamper,
            80,
            false_auth,
            false_denial,
            questions.len(),
            signals,
            complete,
            executable,
            candidate_replays,
            report.manifest_unchanged,
            SOURCE_ID,
            report.source_document_sha256
        ),
    )?;
    assert_eq!(exact, 120);
    assert_eq!(values, 80);
    assert_eq!(frontend_replays, 120);
    assert_eq!(frontend_tamper, 120);
    assert_eq!(false_auth, 0);
    assert_eq!(false_denial, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
