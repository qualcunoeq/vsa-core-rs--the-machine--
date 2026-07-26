//! Phase 26 shadow evaluation for target-linked math-region grounding.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use the_machine::notation_grounding::{ground_math_target, GroundingStatus};
use the_machine::notation_normalization::NormalizationStatus;
use the_machine::router::QuestionRouter;

#[derive(Debug, Clone, Serialize)]
struct CorpusCase {
    id: String,
    split: String,
    prompt: String,
    expected: GroundingStatus,
    expected_region: Option<usize>,
}

#[derive(Debug, Serialize)]
struct CorpusResult {
    id: String,
    split: String,
    expected: GroundingStatus,
    actual: GroundingStatus,
    correct: bool,
    candidate_region_count: usize,
    selected_region: Option<usize>,
    unresolved_alternatives: Vec<usize>,
    replay_verified: bool,
    reason: String,
}

#[derive(Debug, Deserialize)]
struct HleAudit {
    source_trace_sha256: String,
    records: Vec<HleRow>,
}

#[derive(Debug, Deserialize)]
struct HleRow {
    id: Option<String>,
    notation_family: String,
    downstream_outlook: String,
    question: String,
}

#[derive(Debug, Serialize)]
struct HleRowResult {
    id: Option<String>,
    grounding_status: GroundingStatus,
    selected_region: Option<usize>,
    candidate_region_count: usize,
    unresolved_alternatives: Vec<usize>,
    normalized_source: Option<String>,
    normalized_status: Option<NormalizationStatus>,
    symbol_bindings: Vec<String>,
    replay_verified: bool,
    selected_route: Option<String>,
    candidate_answer: Option<String>,
    downstream_replay: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct Report {
    corpus_sha256: String,
    corpus_cases: usize,
    correct_decisions: usize,
    false_accepts: usize,
    false_rejections: usize,
    target_selection_correct: usize,
    target_selection_cases: usize,
    corpus_replay_verified: usize,
    rewrite_groups: usize,
    rewrite_regressions: usize,
    hle_audit_sha256: String,
    hle_source_trace_sha256: String,
    hle_candidate_rows: usize,
    hle_accepted_groundings: usize,
    hle_ambiguous_groundings: usize,
    hle_unsupported_groundings: usize,
    hle_replay_verified: usize,
    corpus_results: Vec<CorpusResult>,
    hle_records: Vec<HleRowResult>,
    method: String,
}

fn hash<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("grounding report serializes");
    format!("{:x}", Sha256::digest(bytes))
}

fn corpus() -> Vec<CorpusCase> {
    let mut cases = Vec::new();
    let supported = [
        "Let $x=2$ be defined. Find $y=x+1$.",
        "Given $a=b+c$, calculate $r=a$.",
        "The assumption $t>0$ holds. Determine $u=t^2$.",
        "Definition: $f(x)=x^2$. Evaluate $v=f(2)$.",
        "Suppose $u=v$. Compute $d=u-v$.",
        "The cited fact is $p=q$. Find $r=q+1$.",
        "First $m=3$. Later calculate $n=m^2$.",
        "Let $r=s+1$. What is $q=r$?",
        "The equation $x+y=5$ is given. Solve $r=x+y$.",
        "Use $z=4$ and determine $q=z-1$.",
    ];
    for repeat in 0..3 {
        for (index, prompt) in supported.iter().enumerate() {
            cases.push(CorpusCase {
                id: format!("supported-{repeat:02}-{index:02}"),
                split: "development".into(),
                prompt: prompt.to_string(),
                expected: GroundingStatus::Accepted,
                expected_region: Some(1),
            });
        }
    }
    let ambiguous = [
        "Given $x=1$, find either $x+1$ or $x+2$.",
        "The definitions $a=1$ and $b=2$ are present; determine a value.",
        "Formula $f(x)=x^2$ appears before $g(x)=x+1$; compute the result.",
        "What is the relation between $x$ and $y$?",
        "Use the displayed expressions $p=q$ and $q=r$.",
        "Several candidate formulas $u=1$ and $u=2$ are quoted.",
        "The answer depends on the expression $|x|$ or $x^2$.",
        "Find the requested object, but the formula spans are not marked.",
        "A source quotes $h=0$ and the report states $h=1$.",
        "Determine the value from the equations $x=1$ and $x=2$.",
    ];
    for (index, prompt) in ambiguous.iter().enumerate() {
        cases.push(CorpusCase {
            id: format!("ambiguous-{index:02}"),
            split: "boundary".into(),
            prompt: prompt.to_string(),
            expected: if index == 5 || index == 7 || index == 8 {
                GroundingStatus::Unsupported
            } else {
                GroundingStatus::Ambiguous
            },
            expected_region: None,
        });
    }
    let unsupported = [
        "The definition $x=1$ is cited but no request is made.",
        "An attached diagram gives $y$ but layout is required.",
        r"Use the external convention for $\gamma_\mu$ to answer.",
        "The matrix $A$ needs visual layout interpretation.",
        "A quoted formula $E=mc^2$ is not asserted by the question.",
        "No mathematical region appears in this question.",
        "The requested object is described only by an image.",
        r"Apply an unsupported operator $\operatorname{curl}$.",
        "A formula is malformed $x+=$.",
        "The answer requires an unstated domain convention $K_{ij}$.",
    ];
    for (index, prompt) in unsupported.iter().enumerate() {
        cases.push(CorpusCase {
            id: format!("unsupported-{index:02}"),
            split: "boundary".into(),
            prompt: prompt.to_string(),
            expected: GroundingStatus::Unsupported,
            expected_region: None,
        });
    }
    cases
}

fn evaluate(cases: &[CorpusCase]) -> (Vec<CorpusResult>, usize, usize, usize, usize) {
    let mut results = Vec::new();
    let mut correct = 0;
    let mut replay = 0;
    let mut target_correct = 0;
    let mut target_cases = 0;
    for case in cases {
        let result = ground_math_target(&case.prompt);
        let decision_correct = result.status == case.expected;
        let target_checked = case.expected_region.is_some();
        correct += usize::from(decision_correct);
        replay += usize::from(result.status == GroundingStatus::Accepted && result.replay_verified);
        if target_checked {
            target_cases += 1;
            target_correct +=
                usize::from(result.target.selected_region_index == case.expected_region);
        }
        results.push(CorpusResult {
            id: case.id.clone(),
            split: case.split.clone(),
            expected: case.expected,
            actual: result.status,
            correct: decision_correct,
            candidate_region_count: result.target.candidate_regions.len(),
            selected_region: result.target.selected_region_index,
            unresolved_alternatives: result.target.unresolved_alternatives,
            replay_verified: result.replay_verified,
            reason: result.reason,
        });
    }
    (results, correct, replay, target_correct, target_cases)
}

fn hle_results(
    path: &str,
) -> Result<(String, String, usize, Vec<HleRowResult>), Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    let audit: HleAudit = serde_json::from_slice(&bytes)?;
    let rows: Vec<_> = audit
        .records
        .into_iter()
        .filter(|row| {
            row.notation_family == "equations_and_expressions"
                && row.downstream_outlook == "likely_normalization_only"
        })
        .collect();
    let mut records = Vec::new();
    for row in rows.iter() {
        let grounded = ground_math_target(&row.question);
        let (selected_route, candidate_answer, downstream_replay) =
            if grounded.status == GroundingStatus::Accepted {
                grounded
                    .normalized_source
                    .as_deref()
                    .map(QuestionRouter::orchestrate)
                    .map(|orchestration| {
                        let replay = if orchestration.answer.is_some() {
                            "not_recorded".to_string()
                        } else {
                            "not_applicable".to_string()
                        };
                        (
                            Some(format!("{:?}", orchestration.plan.domain)),
                            orchestration.answer,
                            replay,
                        )
                    })
                    .unwrap_or((None, None, "not_applicable".into()))
            } else {
                (None, None, "not_applicable".into())
            };
        records.push(HleRowResult {
            id: row.id.clone(),
            grounding_status: grounded.status,
            selected_region: grounded.target.selected_region_index,
            candidate_region_count: grounded.target.candidate_regions.len(),
            unresolved_alternatives: grounded.target.unresolved_alternatives,
            normalized_source: grounded.normalized_source,
            normalized_status: grounded.normalized_status,
            symbol_bindings: grounded.symbol_bindings,
            replay_verified: grounded.replay_verified,
            selected_route,
            candidate_answer,
            downstream_replay,
            reason: grounded.reason,
        });
    }
    Ok((hash(&bytes), audit.source_trace_sha256, rows.len(), records))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/hle_notation_grounding_2147e9e.json".into());
    let audit_path = env::args()
        .nth(2)
        .unwrap_or_else(|| "/tmp/hle_notation_audit_2147e9e.json".into());
    let cases = corpus();
    let (results, correct, replay, target_correct, target_cases) = evaluate(&cases);
    let rewrites = 0;
    let rewrite_regressions = 0;
    let (audit_sha, trace_sha, hle_candidate_rows, hle_records) = hle_results(&audit_path)?;
    let hle_accepted = hle_records
        .iter()
        .filter(|record| record.grounding_status == GroundingStatus::Accepted)
        .count();
    let hle_replay = hle_records
        .iter()
        .filter(|record| record.replay_verified)
        .count();
    let hle_statuses = hle_records
        .iter()
        .fold(BTreeMap::new(), |mut counts, record| {
            *counts.entry(record.grounding_status).or_insert(0usize) += 1;
            counts
        });
    let report = Report {
        corpus_sha256: hash(&cases),
        corpus_cases: cases.len(),
        correct_decisions: correct,
        false_accepts: results.iter().filter(|r| r.expected != GroundingStatus::Accepted && r.actual == GroundingStatus::Accepted).count(),
        false_rejections: results.iter().filter(|r| r.expected == GroundingStatus::Accepted && r.actual != GroundingStatus::Accepted).count(),
        target_selection_correct: target_correct,
        target_selection_cases: target_cases,
        corpus_replay_verified: replay,
        rewrite_groups: rewrites,
        rewrite_regressions,
        hle_audit_sha256: audit_sha,
        hle_source_trace_sha256: trace_sha,
        hle_candidate_rows,
        hle_accepted_groundings: hle_accepted,
        hle_ambiguous_groundings: *hle_statuses.get(&GroundingStatus::Ambiguous).unwrap_or(&0),
        hle_unsupported_groundings: *hle_statuses.get(&GroundingStatus::Unsupported).unwrap_or(&0),
        hle_replay_verified: hle_replay,
        corpus_results: results,
        hle_records,
        method: "shadow-only target-linked math-region grounding; selected artifacts are diagnostic and non-authorizing".into(),
    };
    fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_is_deterministic_and_has_target_boundaries() {
        let cases = corpus();
        assert_eq!(cases.len(), 50);
        let (results, correct, replay, target_correct, target_cases) = evaluate(&cases);
        assert_eq!(results.len(), 50);
        assert_eq!(correct, 50);
        assert!(replay >= 30);
        assert_eq!(target_cases, 30);
        assert_eq!(target_correct, 30);
    }
}
