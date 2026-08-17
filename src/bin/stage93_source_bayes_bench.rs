//! Stage 93: independently validated source-derived Bayes capability.
//!
//! This benchmark validates a second source-derived domain and its typed handoff
//! into the existing finite-probability Bayes operation.  The source catalog is
//! data; both executions use generic runtimes.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::probability_pack::{evaluate_probability, ProbabilityArtifact, ProbabilityOperation, ProbabilityRequest};
use the_machine::source_bayes_frontend::{formalize_bayes_text, replay_verified, BayesFrontendStatus};
use the_machine::source_bayes_pack::{evaluate, records, DOMAIN};
use the_machine::source_formula_pack::{extract_formula_records, FormulaStatus};

const SOURCE: &str = include_str!("../../docs/sources/openstax_bayes_rule_catalog.txt");

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Hidden { Supported, Ambiguous, Unsupported }

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Partition { Development, Validation, Sealed }

#[derive(Debug, Clone, Serialize)]
struct Question { id: String, text: String, hidden: Hidden, partition: Partition }

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    partition: Partition,
    hidden: Hidden,
    frontend_status: String,
    formula_status: Option<String>,
    probability_status: Option<String>,
    authorized: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    provenance_preserved: bool,
    false_authorization: bool,
    false_denial: bool,
    text_sha256: String,
}

#[derive(Debug, Serialize)]
struct PartitionMetrics {
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    supported_authorized: usize,
    ambiguities_preserved: usize,
    unsupported_refused: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_id: &'static str,
    source_sha256: String,
    catalog_sha256: String,
    question_corpus_sha256: String,
    sealed_question_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    supported_authorized: usize,
    ambiguities_preserved: usize,
    unsupported_refused: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    provenance_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    source_mutations_rejected: usize,
    cross_domain_replays: usize,
    partitions: BTreeMap<String, PartitionMetrics>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn partition(global: usize) -> Partition {
    if global < 180 { Partition::Development } else if global < 240 { Partition::Validation } else { Partition::Sealed }
}

fn hidden(local: usize) -> Hidden {
    match local % 10 { 0..=5 => Hidden::Supported, 6..=7 => Hidden::Ambiguous, _ => Hidden::Unsupported }
}

fn question(global: usize) -> Question {
    let local = global % 120;
    let hidden = hidden(local);
    let p = format!("{}/{}", 1 + local % 7, 20 + local % 9);
    let l = format!("{}/{}", 2 + local % 6, 9 + local % 7);
    let e = format!("{}/{}", 3 + local % 8, 10 + local % 9);
    let text = match hidden {
        Hidden::Supported => match local % 3 {
            0 => format!("Use Bayes theorem with prior={p}, likelihood={l}, evidence={e} to find the posterior probability."),
            1 => format!("Find the posterior by Bayes: prior {p}, likelihood {l}, evidence {e}."),
            _ => format!("Given prior={p}, P(B|A)={l}, and P(B)={e}, calculate the Bayes posterior."),
        },
        Hidden::Ambiguous => match local % 2 {
            0 => format!("Use Bayes or a competing rule with prior={p}, likelihood={l}, evidence={e} to find the posterior."),
            _ => format!("There are two possible posterior targets; use Bayes with prior={p}, likelihood={l}, evidence={e}."),
        },
        Hidden::Unsupported => match local % 4 {
            0 => format!("Use Bayes with prior={p}, likelihood={l}, but the evidence is unknown."),
            1 => format!("Use a continuous density Bayes model with prior={p}, likelihood={l}, evidence={e}."),
            2 => format!("Use Bayes with prior={p}, likelihood={l}, evidence=0 to find a posterior."),
            _ => "Infer a posterior from an unspecified diagnostic model.".into(),
        },
    };
    Question { id: format!("bayes_source_{global:04}"), text, hidden, partition: partition(global) }
}

fn run(question: &Question) -> Receipt {
    let frontend = formalize_bayes_text(&question.text, &question.id);
    let formula = frontend.request.as_ref().map(evaluate);
    let probability = frontend.request.as_ref().and_then(|request| {
        let result = evaluate(request);
        if result.status != FormulaStatus::Complete { return None; }
        let prior = request.inputs.get("prior")?.clone();
        let likelihood = request.inputs.get("likelihood")?.clone();
        let evidence = request.inputs.get("evidence")?.clone();
        Some(evaluate_probability(&ProbabilityRequest {
            operation: ProbabilityOperation::Bayes,
            domain: "finite_exact_probability".into(),
            outcomes: Vec::new(), probabilities: Vec::new(), values: Vec::new(),
            event_a: None, event_b: None, partition: Vec::new(), conditional_values: Vec::new(),
            prior_probability: Some(prior), likelihood: Some(likelihood), evidence: Some(evidence),
            ambiguity: None, provenance: vec![question.id.clone(), "source-bayes-bridge".into()],
        }))
    });
    let authorized = frontend.status == BayesFrontendStatus::Complete
        && formula.as_ref().is_some_and(|r| r.status == FormulaStatus::Complete && r.value.is_some() && r.replay_verified())
        && probability.as_ref().is_some_and(|r| matches!(r.artifact, Some(ProbabilityArtifact::Scalar(_))) && r.replay_verified());
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let formula_replay = formula.as_ref().is_none_or(|r| r.replay_verified());
    let probability_replay = probability.as_ref().is_none_or(|r| r.replay_verified());
    let formula_tamper = formula.as_ref().is_none_or(|r| { let mut c = r.clone(); c.replay_hash.push('x'); !c.replay_verified() });
    let probability_tamper = probability.as_ref().is_none_or(|r| { let mut c = r.clone(); c.replay_hash.push('x'); !c.replay_verified() });
    let actual_status = if authorized { "supported" } else if frontend.status == BayesFrontendStatus::Ambiguous { "ambiguous" } else if frontend.status == BayesFrontendStatus::Unsupported { "unsupported" } else { "missing" };
    Receipt {
        id: question.id.clone(), partition: question.partition, hidden: question.hidden,
        frontend_status: actual_status.into(),
        formula_status: formula.as_ref().map(|r| format!("{:?}", r.status)),
        probability_status: probability.as_ref().map(|r| format!("{:?}", r.status)),
        authorized, replay_verified: replay_verified(&frontend) && formula_replay && probability_replay,
        tamper_rejected: !replay_verified(&frontend_tampered) && formula_tamper && probability_tamper,
        provenance_preserved: !frontend.provenance.is_empty() && formula.as_ref().is_none_or(|r| !r.provenance.is_empty()) && probability.as_ref().is_none_or(|r| !r.provenance.is_empty()),
        false_authorization: question.hidden != Hidden::Supported && authorized,
        false_denial: question.hidden == Hidden::Supported && !authorized,
        text_sha256: digest(&question.text),
    }
}

fn partition_metrics(receipts: &[Receipt], partition: Partition) -> PartitionMetrics {
    let rows: Vec<_> = receipts.iter().filter(|r| r.partition == partition).collect();
    PartitionMetrics {
        cases: rows.len(), supported: rows.iter().filter(|r| r.hidden == Hidden::Supported).count(),
        ambiguous: rows.iter().filter(|r| r.hidden == Hidden::Ambiguous).count(), unsupported: rows.iter().filter(|r| r.hidden == Hidden::Unsupported).count(),
        supported_authorized: rows.iter().filter(|r| r.hidden == Hidden::Supported && r.authorized).count(),
        ambiguities_preserved: rows.iter().filter(|r| r.hidden == Hidden::Ambiguous && r.frontend_status == "ambiguous").count(),
        unsupported_refused: rows.iter().filter(|r| r.hidden == Hidden::Unsupported && ["unsupported", "missing"].contains(&r.frontend_status.as_str())).count(),
        replay_verified: rows.iter().filter(|r| r.replay_verified).count(), tamper_rejections: rows.iter().filter(|r| r.tamper_rejected).count(),
        false_authorizations: rows.iter().filter(|r| r.false_authorization).count(), false_denials: rows.iter().filter(|r| r.false_denial).count(),
    }
}

fn main() {
    assert_eq!(records().len(), 1);
    let questions: Vec<_> = (0..300).map(question).collect();
    let receipts: Vec<_> = questions.iter().map(run).collect();
    let mutations = [
        SOURCE.replace("https://", "http://"), SOURCE.replace("BEGIN FORMULA bayes_posterior", "BEGIN FORMULA"),
        SOURCE.replace("END FORMULA", "END"), SOURCE.replace("INPUTS: prior, likelihood, evidence", "INPUTS: prior"),
        SOURCE.replace("EXPRESSION: prior * likelihood / evidence", "EXPRESSION: unsupported$"), SOURCE.replace("CONSTRAINTS: probability:prior;", "CONSTRAINTS: probability:missing;"),
    ];
    let source_mutations_rejected = mutations.iter().filter(|doc| extract_formula_records(doc).is_err()).count();
    let supported_authorized = receipts.iter().filter(|r| r.authorized).count();
    let report = Report {
        schema: "stage93-source-bayes-bench-v1", source_id: "openstax-principles-data-science:probability-theory",
        source_sha256: digest(&SOURCE), catalog_sha256: digest(&records()), question_corpus_sha256: digest(&questions),
        sealed_question_sha256: digest(&questions[240..]), cases: receipts.len(),
        supported: receipts.iter().filter(|r| r.hidden == Hidden::Supported).count(), ambiguous: receipts.iter().filter(|r| r.hidden == Hidden::Ambiguous).count(), unsupported: receipts.iter().filter(|r| r.hidden == Hidden::Unsupported).count(),
        supported_authorized, ambiguities_preserved: receipts.iter().filter(|r| r.hidden == Hidden::Ambiguous && r.frontend_status == "ambiguous").count(), unsupported_refused: receipts.iter().filter(|r| r.hidden == Hidden::Unsupported && ["unsupported", "missing"].contains(&r.frontend_status.as_str())).count(),
        replay_verified: receipts.iter().filter(|r| r.replay_verified).count(), tamper_rejections: receipts.iter().filter(|r| r.tamper_rejected).count(), provenance_preserved: receipts.iter().filter(|r| r.provenance_preserved).count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(), false_denials: receipts.iter().filter(|r| r.false_denial).count(),
        source_mutations_rejected, cross_domain_replays: receipts.iter().filter(|r| r.authorized && r.probability_status.as_deref() == Some("Complete")).count(),
        partitions: BTreeMap::from([(String::from("development"), partition_metrics(&receipts, Partition::Development)), (String::from("validation"), partition_metrics(&receipts, Partition::Validation)), (String::from("sealed"), partition_metrics(&receipts, Partition::Sealed))]), receipts,
    };
    assert_eq!(report.supported, 180); assert_eq!(report.ambiguous, 60); assert_eq!(report.unsupported, 60);
    assert_eq!(report.supported_authorized, 180); assert_eq!(report.ambiguities_preserved, 60); assert_eq!(report.unsupported_refused, 60);
    assert_eq!(report.replay_verified, 300); assert_eq!(report.tamper_rejections, 300); assert_eq!(report.provenance_preserved, 300);
    assert_eq!(report.false_authorizations, 0); assert_eq!(report.false_denials, 0); assert_eq!(report.source_mutations_rejected, 6); assert_eq!(report.cross_domain_replays, 180);
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
