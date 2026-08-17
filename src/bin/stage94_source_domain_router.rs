//! Stage 94: mixed source-domain routing and cross-domain transfer.
//!
//! The corpus deliberately interleaves two source-derived domains.  A route is
//! authorized only when exactly one bounded frontend produces a complete typed
//! request and its generic execution plus replay succeeds.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::probability_pack::{evaluate_probability, ProbabilityArtifact, ProbabilityOperation, ProbabilityRequest};
use the_machine::source_bayes_frontend::{formalize_bayes_text, replay_verified as bayes_replay, BayesFrontendStatus};
use the_machine::source_formula_pack::{evaluate_formula_records, extract_formula_records, FormulaStatus};
use the_machine::source_interpolation_frontend::{formalize_interpolation_text, replay_verified as interpolation_replay, InterpolationFrontendStatus};

const INTERPOLATION_SOURCE: &str = include_str!("../../docs/sources/openstax_linear_interpolation_catalog.txt");
const BAYES_SOURCE: &str = include_str!("../../docs/sources/openstax_bayes_rule_catalog.txt");
const INTERPOLATION_DOMAIN: &str = "source_catalog_linear_interpolation";
const BAYES_DOMAIN: &str = "source_catalog_bayes_rule";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Expected { Interpolation, Bayes, Ambiguous, Unsupported }

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Partition { Development, Validation, Sealed }

#[derive(Debug, Clone, Serialize)]
struct Case { id: String, text: String, expected: Expected, partition: Partition }

#[derive(Debug, Clone, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    selected_route: String,
    complete_routes: usize,
    authorized: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    provenance_preserved: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Metrics {
    cases: usize,
    interpolation: usize,
    bayes: usize,
    ambiguous: usize,
    unsupported: usize,
    route_correct: usize,
    authorized: usize,
    ambiguity_preserved: usize,
    unsupported_refused: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    provenance_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    interpolation_source_sha256: String,
    bayes_source_sha256: String,
    interpolation_catalog_sha256: String,
    bayes_catalog_sha256: String,
    corpus_sha256: String,
    sealed_sha256: String,
    metrics: Metrics,
    partitions: BTreeMap<String, Metrics>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String { format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap())) }

fn partition(index: usize) -> Partition {
    match index % 5 {
        0..=2 => Partition::Development,
        3 => Partition::Validation,
        _ => Partition::Sealed,
    }
}

fn case(index: usize) -> Case {
    let local = index % 120;
    let kind = index / 120;
    let (expected, text) = match kind {
        0 => {
            let x1 = 1 + local % 4; let x2 = x1 + 6 + local % 5; let target = x1 + (x2 - x1) / 2;
            let y1 = 4 + local % 9; let y2 = y1 + 8 + local % 11;
            (Expected::Interpolation, format!("Linearly interpolate at x={target} between x1={x1},y1={y1} and x2={x2},y2={y2}; prior probability is irrelevant."))
        }
        1 => {
            let p = format!("{}/{}", 1 + local % 7, 20 + local % 9); let l = format!("{}/{}", 2 + local % 6, 9 + local % 7); let e = format!("{}/{}", 3 + local % 8, 10 + local % 9);
            (Expected::Bayes, format!("Use Bayes with prior={p}, likelihood={l}, evidence={e}; interpolation is irrelevant."))
        }
        2 => {
            if local % 2 == 0 {
                (Expected::Ambiguous, "Interpolate or extrapolate at x=6 between x1=1,y1=4 and x2=11,y2=24.".into())
            } else {
                (Expected::Ambiguous, "Use Bayes or another rule with prior=1/4, likelihood=1/2, evidence=1/3.".into())
            }
        }
        _ => match local % 4 {
            0 => (Expected::Unsupported, "Use quadratic spline interpolation at x=6 through x1=1,y1=4 and x2=11,y2=24.".into()),
            1 => (Expected::Unsupported, "Use Bayes with prior=1/4 and an unknown evidence probability.".into()),
            2 => (Expected::Unsupported, "Apply a continuous density model with prior=1/4, likelihood=1/2, evidence=1/3.".into()),
            _ => (Expected::Unsupported, "A qualitative scientific claim has no bounded interpolation or Bayes request.".into()),
        },
    };
    Case { id: format!("mixed_source_route_{index:04}"), text, expected, partition: partition(index) }
}

fn run(case: &Case, interpolation_records: &[the_machine::source_formula_pack::FormulaRecord], bayes_records: &[the_machine::source_formula_pack::FormulaRecord]) -> Receipt {
    let interpolation = formalize_interpolation_text(&case.text, &case.id);
    let bayes = formalize_bayes_text(&case.text, &case.id);
    let interpolation_complete = interpolation.status == InterpolationFrontendStatus::Complete && interpolation.request.as_ref().is_some_and(|r| evaluate_formula_records(r, INTERPOLATION_DOMAIN, interpolation_records).status == FormulaStatus::Complete);
    let bayes_complete = bayes.status == BayesFrontendStatus::Complete && bayes.request.as_ref().is_some_and(|r| evaluate_formula_records(r, BAYES_DOMAIN, bayes_records).status == FormulaStatus::Complete);
    let complete_routes = usize::from(interpolation_complete) + usize::from(bayes_complete);
    let selected_route = match (interpolation_complete, bayes_complete) { (true, false) => "interpolation", (false, true) => "bayes", _ => "none" };
    let authorized = match selected_route {
        "interpolation" => interpolation.request.as_ref().is_some_and(|r| evaluate_formula_records(r, INTERPOLATION_DOMAIN, interpolation_records).replay_verified()),
        "bayes" => {
            let Some(request) = bayes.request.as_ref() else { return Receipt { id: case.id.clone(), expected: case.expected, selected_route: "none".into(), complete_routes, authorized: false, replay_verified: false, tamper_rejected: false, provenance_preserved: false, false_authorization: false, false_denial: case.expected == Expected::Interpolation || case.expected == Expected::Bayes }; };
            let formula = evaluate_formula_records(request, BAYES_DOMAIN, bayes_records);
            let probability = evaluate_probability(&ProbabilityRequest { operation: ProbabilityOperation::Bayes, domain: "finite_exact_probability".into(), outcomes: Vec::new(), probabilities: Vec::new(), values: Vec::new(), event_a: None, event_b: None, partition: Vec::new(), conditional_values: Vec::new(), prior_probability: request.inputs.get("prior").cloned(), likelihood: request.inputs.get("likelihood").cloned(), evidence: request.inputs.get("evidence").cloned(), ambiguity: None, provenance: vec![case.id.clone(), "mixed-domain-route".into()] });
            formula.replay_verified() && probability.replay_verified() && matches!(probability.artifact, Some(ProbabilityArtifact::Scalar(_)))
        }
        _ => false,
    };
    let route_correct = matches!((case.expected, selected_route), (Expected::Interpolation, "interpolation") | (Expected::Bayes, "bayes") | (Expected::Ambiguous, "none") | (Expected::Unsupported, "none"));
    let mut interpolation_tampered = interpolation.clone(); interpolation_tampered.replay_hash.push('x');
    let mut bayes_tampered = bayes.clone(); bayes_tampered.replay_hash.push('x');
    let replay_verified = interpolation_replay(&interpolation) && bayes_replay(&bayes);
    let tamper_rejected = !interpolation_replay(&interpolation_tampered) && !bayes_replay(&bayes_tampered);
    Receipt { id: case.id.clone(), expected: case.expected, selected_route: selected_route.into(), complete_routes, authorized, replay_verified, tamper_rejected, provenance_preserved: !interpolation.provenance.is_empty() && !bayes.provenance.is_empty(), false_authorization: !matches!(case.expected, Expected::Interpolation | Expected::Bayes) && authorized, false_denial: matches!(case.expected, Expected::Interpolation | Expected::Bayes) && !authorized, }
}

fn metrics(receipts: &[Receipt]) -> Metrics {
    Metrics { cases: receipts.len(), interpolation: receipts.iter().filter(|r| r.expected == Expected::Interpolation).count(), bayes: receipts.iter().filter(|r| r.expected == Expected::Bayes).count(), ambiguous: receipts.iter().filter(|r| r.expected == Expected::Ambiguous).count(), unsupported: receipts.iter().filter(|r| r.expected == Expected::Unsupported).count(), route_correct: receipts.iter().filter(|r| matches!((r.expected, r.selected_route.as_str()), (Expected::Interpolation, "interpolation") | (Expected::Bayes, "bayes") | (Expected::Ambiguous, "none") | (Expected::Unsupported, "none"))).count(), authorized: receipts.iter().filter(|r| r.authorized).count(), ambiguity_preserved: receipts.iter().filter(|r| r.expected == Expected::Ambiguous && r.selected_route == "none").count(), unsupported_refused: receipts.iter().filter(|r| r.expected == Expected::Unsupported && r.selected_route == "none").count(), replay_verified: receipts.iter().filter(|r| r.replay_verified).count(), tamper_rejections: receipts.iter().filter(|r| r.tamper_rejected).count(), provenance_preserved: receipts.iter().filter(|r| r.provenance_preserved).count(), false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(), false_denials: receipts.iter().filter(|r| r.false_denial).count(), route_leakage: receipts.iter().filter(|r| r.complete_routes > 1).count() }
}

fn main() {
    let interpolation_records = extract_formula_records(INTERPOLATION_SOURCE).expect("interpolation source validates");
    let bayes_records = extract_formula_records(BAYES_SOURCE).expect("Bayes source validates");
    let cases: Vec<_> = (0..480).map(case).collect();
    let receipts: Vec<_> = cases.iter().map(|c| run(c, &interpolation_records, &bayes_records)).collect();
    let overall = metrics(&receipts);
    assert_eq!(overall.cases, 480); assert_eq!(overall.interpolation, 120); assert_eq!(overall.bayes, 120); assert_eq!(overall.ambiguous, 120); assert_eq!(overall.unsupported, 120);
    assert_eq!(overall.route_correct, 480); assert_eq!(overall.authorized, 240); assert_eq!(overall.ambiguity_preserved, 120); assert_eq!(overall.unsupported_refused, 120);
    assert_eq!(overall.replay_verified, 480); assert_eq!(overall.tamper_rejections, 480); assert_eq!(overall.provenance_preserved, 480); assert_eq!(overall.false_authorizations, 0); assert_eq!(overall.false_denials, 0); assert_eq!(overall.route_leakage, 0);
    let development: Vec<_> = receipts.iter().filter(|r| cases.iter().find(|c| c.id == r.id).is_some_and(|c| c.partition == Partition::Development)).cloned().collect();
    let validation: Vec<_> = receipts.iter().filter(|r| cases.iter().find(|c| c.id == r.id).is_some_and(|c| c.partition == Partition::Validation)).cloned().collect();
    let sealed: Vec<_> = receipts.iter().filter(|r| cases.iter().find(|c| c.id == r.id).is_some_and(|c| c.partition == Partition::Sealed)).cloned().collect();
    let report = Report { schema: "stage94-source-domain-router-v1", interpolation_source_sha256: digest(&INTERPOLATION_SOURCE), bayes_source_sha256: digest(&BAYES_SOURCE), interpolation_catalog_sha256: digest(&interpolation_records), bayes_catalog_sha256: digest(&bayes_records), corpus_sha256: digest(&cases), sealed_sha256: digest(&sealed), metrics: overall, partitions: BTreeMap::from([(String::from("development"), metrics(&development)), (String::from("validation"), metrics(&validation)), (String::from("sealed"), metrics(&sealed))]), receipts };
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
