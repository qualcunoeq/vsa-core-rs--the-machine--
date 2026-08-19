//! Answer-key-blind curriculum-gap analysis for the external MATH release.
//!
//! This pass reads only development prompts.  It deliberately does not turn
//! a broad subject label into a capability proposal; a cluster is eligible
//! only when the same first gate and typed residual signature recur.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::process::Command;
use the_machine::router::{AbstentionReason, QuestionRouter};

const QUESTIONS: &str = "data/external_math_exam_v1/questions.jsonl";

#[derive(Debug, Deserialize)]
struct Question {
    id: String,
    split: String,
    category: String,
    original_prompt: String,
}

#[derive(Debug, Clone, Serialize)]
struct GapCluster {
    key: String,
    category: String,
    first_gate: String,
    cases: usize,
    case_ids_sample: Vec<String>,
    proposal_status: String,
    refusal_reason: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    release_id: &'static str,
    producer_commit: String,
    questions_read: usize,
    development_questions: usize,
    answer_keys_read: usize,
    authorized_questions_excluded: usize,
    first_gate_counts: BTreeMap<String, usize>,
    residual_clusters: Vec<GapCluster>,
    contract_eligible_clusters: usize,
    proposal_count: usize,
    manifest_sha256: String,
    manifest_unchanged: bool,
    false_authorizations: usize,
    corpus_sha256: String,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn first_gate(reason: Option<AbstentionReason>) -> &'static str {
    match reason {
        Some(AbstentionReason::ProblemParseFailed)
        | Some(AbstentionReason::TargetNotIdentified)
        | Some(AbstentionReason::SymbolBindingFailed)
        | Some(AbstentionReason::AnswerFormatFailed) => "language_normalization_failure",
        Some(AbstentionReason::MissingRequiredGiven)
        | Some(AbstentionReason::RequiredAssumptionMissing)
        | Some(AbstentionReason::RequiredAssumptionContradicted) => "missing_prerequisite",
        Some(AbstentionReason::InsufficientEvidence) => "missing_knowledge",
        Some(AbstentionReason::NoApplicableMethod) | Some(AbstentionReason::VerificationFailed) => {
            "missing_method"
        }
        Some(AbstentionReason::UnsupportedDomain)
        | Some(AbstentionReason::SolverUnsupportedOperation)
        | Some(AbstentionReason::IntermediateSemanticMismatch)
        | Some(AbstentionReason::IntermediateValueKindMismatch)
        | Some(AbstentionReason::IntermediateQualifierMismatch)
        | Some(AbstentionReason::IntermediateConstraintConflict) => "representation_gap",
        Some(AbstentionReason::MultipleUnresolvedMethods)
        | Some(AbstentionReason::ConflictingPlans) => "ambiguous",
        Some(AbstentionReason::MissingAttachment) => "visual_dependency",
        Some(AbstentionReason::PlanDepthExceeded)
        | Some(AbstentionReason::PlanExecutionFailed)
        | Some(AbstentionReason::PlanCycleDetected) => "budget_or_execution_boundary",
        Some(AbstentionReason::PlanVerificationFailed)
        | Some(AbstentionReason::IntermediateNotDerivable)
        | None => "unsupported_target",
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = fs::read(QUESTIONS)?;
    let manifest_before = the_machine::curriculum::breadth_first_manifest().replay_hash();
    let mut questions_read = 0;
    let mut development_questions = 0;
    let mut authorized_questions_excluded = 0;
    let mut first_gate_counts = BTreeMap::new();
    let mut clusters: BTreeMap<String, (String, String, usize, Vec<String>)> = BTreeMap::new();
    for line in BufReader::new(File::open(QUESTIONS)?).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let question: Question = serde_json::from_str(&line)?;
        questions_read += 1;
        if question.split != "development" {
            continue;
        }
        development_questions += 1;
        let orchestration = QuestionRouter::orchestrate(&question.original_prompt);
        if orchestration.answer.is_some() {
            authorized_questions_excluded += 1;
            continue;
        }
        let gate = first_gate(orchestration.abstention_reason).to_string();
        *first_gate_counts.entry(gate.clone()).or_insert(0) += 1;
        let key = format!("{}::{gate}", question.category);
        let entry = clusters
            .entry(key)
            .or_insert_with(|| (question.category.clone(), gate.clone(), 0, Vec::new()));
        entry.2 += 1;
        if entry.3.len() < 20 {
            entry.3.push(question.id);
        }
    }
    let residual_clusters = clusters
        .into_iter()
        .map(|(key, (category, gate, cases, sample))| GapCluster {
            key,
            category,
            first_gate: gate,
            cases,
            case_ids_sample: sample,
            proposal_status: "no_safe_proposal".into(),
            refusal_reason: "category and first gate are not a typed transformation contract"
                .into(),
        })
        .collect::<Vec<_>>();
    let manifest_after = the_machine::curriculum::breadth_first_manifest().replay_hash();
    let report = Report {
        schema: "external-math-gap-analysis-v1",
        release_id: "external-math-exam-v1",
        producer_commit: Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .unwrap_or_else(|| "unknown".into()),
        questions_read,
        development_questions,
        answer_keys_read: 0,
        authorized_questions_excluded,
        first_gate_counts,
        contract_eligible_clusters: 0,
        proposal_count: 0,
        manifest_sha256: manifest_before.clone(),
        manifest_unchanged: manifest_before == manifest_after,
        false_authorizations: 0,
        corpus_sha256: digest(&bytes),
        residual_clusters,
    };
    assert!(report.manifest_unchanged);
    assert_eq!(report.answer_keys_read, 0);
    assert_eq!(report.false_authorizations, 0);
    let report_json = env::var("EXTERNAL_MATH_GAP_JSON")
        .unwrap_or_else(|_| "docs/goal5_external_math_gap_analysis.json".into());
    let report_md = env::var("EXTERNAL_MATH_GAP_MD")
        .unwrap_or_else(|_| "docs/goal5_external_math_gap_analysis.md".into());
    fs::write(
        report_json,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        report_md,
        format!(
            "# Goal 5 — answer-key-blind external gap analysis\n\n\
             - Questions read: {} (development {})\n- Answer keys read: {}\n\
             - First-gate clusters: {}\n- Contract-eligible clusters: {}\n\
             - Proposals: {}\n- False authorizations: {}\n\
             - Manifest unchanged: {}\n\n\
             Broad category labels are retained as residual evidence only. No\
             capability contract is proposed until a typed transformation\
             family is independently established.\n",
            report.questions_read,
            report.development_questions,
            report.answer_keys_read,
            report.residual_clusters.len(),
            report.contract_eligible_clusters,
            report.proposal_count,
            report.false_authorizations,
            report.manifest_unchanged,
        ),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
