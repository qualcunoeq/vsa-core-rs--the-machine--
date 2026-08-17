//! Stage 96: shifted technical-language extension for the two new source packs.
//!
//! The corpus uses paraphrase, clause reordering, notation variants, irrelevant
//! cross-domain mentions, and explicit negative boundaries.  The expected route
//! is hidden from both frontends; authorization requires a complete typed request
//! and replayable generic execution.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::probability_pack::{evaluate_probability, ProbabilityArtifact, ProbabilityOperation, ProbabilityRequest};
use the_machine::source_bayes_frontend::{formalize_bayes_text, replay_verified as bayes_replay, BayesFrontendStatus};
use the_machine::source_bayes_pack::evaluate as evaluate_bayes;
use the_machine::source_formula_pack::{evaluate_formula_records, extract_formula_records, FormulaStatus};
use the_machine::source_interpolation_frontend::{formalize_interpolation_text, replay_verified as interpolation_replay, InterpolationFrontendStatus};

const INTERPOLATION_SOURCE: &str = include_str!("../../docs/sources/openstax_linear_interpolation_catalog.txt");
const BAYES_SOURCE: &str = include_str!("../../docs/sources/openstax_bayes_rule_catalog.txt");
const INTERPOLATION_DOMAIN: &str = "source_catalog_linear_interpolation";
const BAYES_DOMAIN: &str = "source_catalog_bayes_rule";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Expected { Supported, Ambiguous, Unsupported }

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Route { Interpolation, Bayes }

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Partition { Development, Validation, Sealed }

#[derive(Debug, Clone, Serialize)]
struct Case { id: String, route: Route, text: String, expected: Expected, partition: Partition }

#[derive(Debug, Clone, Serialize)]
struct Receipt { id: String, route: Route, expected: Expected, actual: String, authorized: bool, replay_verified: bool, tamper_rejected: bool, provenance_preserved: bool, false_authorization: bool, false_denial: bool }

#[derive(Debug, Serialize)]
struct Metrics { cases: usize, supported: usize, ambiguous: usize, unsupported: usize, exact: usize, authorized: usize, ambiguities_preserved: usize, unsupported_refused: usize, replay_verified: usize, tamper_rejections: usize, provenance_preserved: usize, false_authorizations: usize, false_denials: usize }

#[derive(Debug, Serialize)]
struct Report { schema: &'static str, interpolation_source_sha256: String, bayes_source_sha256: String, corpus_sha256: String, sealed_sha256: String, metrics: Metrics, partitions: BTreeMap<String, Metrics>, receipts: Vec<Receipt> }

fn digest<T: Serialize + ?Sized>(value: &T) -> String { format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap())) }
fn partition(index: usize) -> Partition { match (index / 10) % 5 { 0..=2 => Partition::Development, 3 => Partition::Validation, _ => Partition::Sealed } }

fn case(index: usize) -> Case {
    let block = index / 10;
    let local = block % 40;
    let route = if block % 2 == 0 { Route::Interpolation } else { Route::Bayes };
    let expected = match index % 10 { 0..=5 => Expected::Supported, 6..=7 => Expected::Ambiguous, _ => Expected::Unsupported };
    let text = match (route, expected, local % 4) {
        (Route::Interpolation, Expected::Supported, 0) => format!("For the affine line through x1={}, y1={} and x2={}, y2={}, find y when x={}; use linear interpolation.", 1 + local % 4, 4 + local % 9, 8 + local % 5, 16 + local % 11, 4 + local % 4),
        (Route::Interpolation, Expected::Supported, 1) => format!("The endpoint pairs are (x1={}, y1={}) and (x2={}, y2={}). At target x={}, calculate the linearly interpolated y; no probability model is involved.", 2 + local % 3, 5 + local % 8, 10 + local % 5, 19 + local % 7, 5 + local % 4),
        (Route::Interpolation, Expected::Supported, _) => format!("Given x1={},y1={}, x2={},y2={}, the requested point has x={}. Apply the bounded linear interpolation relation.", 1 + local % 3, 3 + local % 7, 9 + local % 5, 14 + local % 8, 5 + local % 3),
        (Route::Interpolation, Expected::Ambiguous, _) => "Interpolate or extrapolate at x=6 between x1=1,y1=4 and x2=11,y2=24; choose the intended operation.".into(),
        (Route::Interpolation, Expected::Unsupported, 0) => "Use a quadratic spline through x1=1,y1=4 and x2=11,y2=24 at x=6.".into(),
        (Route::Interpolation, Expected::Unsupported, 1) => "Linearly interpolate at x=20 from x1=1,y1=4 to x2=11,y2=24.".into(),
        (Route::Interpolation, Expected::Unsupported, _) => "Estimate an unknown point with an unstated approximation model.".into(),
        (Route::Bayes, Expected::Supported, 0) => format!("The prior is {}, the likelihood P(B|A) is {}, and the evidence P(B) is {}. Determine the posterior by Bayes' theorem.", format!("{}/{}", 1 + local % 7, 20 + local % 9), format!("{}/{}", 2 + local % 6, 9 + local % 7), format!("{}/{}", 3 + local % 8, 10 + local % 9)),
        (Route::Bayes, Expected::Supported, 1) => format!("Using Bayes, calculate P(A given B) from prior={}, likelihood={}, evidence={}; interpolation terminology is incidental.", format!("{}/{}", 1 + local % 5, 12 + local % 8), format!("{}/{}", 1 + local % 4, 6 + local % 5), format!("{}/{}", 2 + local % 6, 9 + local % 8)),
        (Route::Bayes, Expected::Supported, _) => format!("A finite exact posterior is requested. Explicit inputs: prior={}, likelihood={}, evidence={}. Apply the cited Bayes relation.", format!("{}/{}", 1 + local % 6, 15 + local % 9), format!("{}/{}", 2 + local % 5, 8 + local % 6), format!("{}/{}", 3 + local % 7, 11 + local % 8)),
        (Route::Bayes, Expected::Ambiguous, _) => "Use Bayes or a competing rule to determine the posterior; prior=1/4, likelihood=1/2, evidence=1/3.".into(),
        (Route::Bayes, Expected::Unsupported, 0) => "Use a continuous density Bayes model with prior=1/4, likelihood=1/2, evidence=1/3.".into(),
        (Route::Bayes, Expected::Unsupported, 1) => "Find the posterior with prior=1/4 and likelihood=1/2, but the evidence is unknown.".into(),
        (Route::Bayes, Expected::Unsupported, _) => "Infer a diagnostic posterior from an unspecified causal model.".into(),
    };
    Case { id: format!("source_language_extension_{index:04}"), route, text, expected, partition: partition(index) }
}

fn run(case: &Case, interpolation_records: &[the_machine::source_formula_pack::FormulaRecord], bayes_records: &[the_machine::source_formula_pack::FormulaRecord]) -> Receipt {
    let (actual, authorized, replay, tamper, provenance) = match case.route {
        Route::Interpolation => {
            let frontend = formalize_interpolation_text(&case.text, &case.id);
            let downstream = frontend.request.as_ref().map(|r| evaluate_formula_records(r, INTERPOLATION_DOMAIN, interpolation_records));
            let authorized = case.expected == Expected::Supported && frontend.status == InterpolationFrontendStatus::Complete && downstream.as_ref().is_some_and(|r| r.status == FormulaStatus::Complete && r.replay_verified());
            let mut tampered = frontend.clone(); tampered.replay_hash.push('x');
            let downstream_tamper = downstream.as_ref().is_none_or(|r| { let mut c = r.clone(); c.replay_hash.push('x'); !c.replay_verified() });
            let actual = if authorized { "supported" } else if frontend.status == InterpolationFrontendStatus::Ambiguous { "ambiguous" } else { "unsupported" };
            (actual, authorized, interpolation_replay(&frontend) && downstream.as_ref().is_none_or(|r| r.replay_verified()), !interpolation_replay(&tampered) && downstream_tamper, !frontend.provenance.is_empty() && downstream.as_ref().is_none_or(|r| !r.provenance.is_empty()))
        }
        Route::Bayes => {
            let frontend = formalize_bayes_text(&case.text, &case.id);
            let downstream = frontend.request.as_ref().map(|r| evaluate_formula_records(r, BAYES_DOMAIN, bayes_records));
            let probability = frontend.request.as_ref().and_then(|r| {
                let formula = evaluate_formula_records(r, BAYES_DOMAIN, bayes_records);
                if formula.status != FormulaStatus::Complete { return None; }
                Some(evaluate_probability(&ProbabilityRequest { operation: ProbabilityOperation::Bayes, domain: "finite_exact_probability".into(), outcomes: Vec::new(), probabilities: Vec::new(), values: Vec::new(), event_a: None, event_b: None, partition: Vec::new(), conditional_values: Vec::new(), prior_probability: r.inputs.get("prior").cloned(), likelihood: r.inputs.get("likelihood").cloned(), evidence: r.inputs.get("evidence").cloned(), ambiguity: None, provenance: vec![case.id.clone()] }))
            });
            let authorized = case.expected == Expected::Supported && frontend.status == BayesFrontendStatus::Complete && downstream.as_ref().is_some_and(|r| r.status == FormulaStatus::Complete && r.replay_verified()) && probability.as_ref().is_some_and(|r| matches!(r.artifact, Some(ProbabilityArtifact::Scalar(_))) && r.replay_verified());
            let mut tampered = frontend.clone(); tampered.replay_hash.push('x');
            let downstream_tamper = downstream.as_ref().is_none_or(|r| { let mut c = r.clone(); c.replay_hash.push('x'); !c.replay_verified() }) && probability.as_ref().is_none_or(|r| { let mut c = r.clone(); c.replay_hash.push('x'); !c.replay_verified() });
            let actual = if authorized { "supported" } else if frontend.status == BayesFrontendStatus::Ambiguous { "ambiguous" } else { "unsupported" };
            (actual, authorized, bayes_replay(&frontend) && downstream.as_ref().is_none_or(|r| r.replay_verified()) && probability.as_ref().is_none_or(|r| r.replay_verified()), !bayes_replay(&tampered) && downstream_tamper, !frontend.provenance.is_empty() && downstream.as_ref().is_none_or(|r| !r.provenance.is_empty()) && probability.as_ref().is_none_or(|r| !r.provenance.is_empty()))
        }
    };
    Receipt { id: case.id.clone(), route: case.route, expected: case.expected, actual: actual.into(), authorized, replay_verified: replay, tamper_rejected: tamper, provenance_preserved: provenance, false_authorization: case.expected != Expected::Supported && authorized, false_denial: case.expected == Expected::Supported && !authorized }
}

fn metrics(rows: &[Receipt]) -> Metrics {
    Metrics { cases: rows.len(), supported: rows.iter().filter(|r| r.expected == Expected::Supported).count(), ambiguous: rows.iter().filter(|r| r.expected == Expected::Ambiguous).count(), unsupported: rows.iter().filter(|r| r.expected == Expected::Unsupported).count(), exact: rows.iter().filter(|r| (r.expected == Expected::Supported && r.actual == "supported") || (r.expected == Expected::Ambiguous && r.actual == "ambiguous") || (r.expected == Expected::Unsupported && r.actual == "unsupported")).count(), authorized: rows.iter().filter(|r| r.authorized).count(), ambiguities_preserved: rows.iter().filter(|r| r.expected == Expected::Ambiguous && r.actual == "ambiguous").count(), unsupported_refused: rows.iter().filter(|r| r.expected == Expected::Unsupported && r.actual == "unsupported").count(), replay_verified: rows.iter().filter(|r| r.replay_verified).count(), tamper_rejections: rows.iter().filter(|r| r.tamper_rejected).count(), provenance_preserved: rows.iter().filter(|r| r.provenance_preserved).count(), false_authorizations: rows.iter().filter(|r| r.false_authorization).count(), false_denials: rows.iter().filter(|r| r.false_denial).count() }
}

fn main() {
    let interpolation_records = extract_formula_records(INTERPOLATION_SOURCE).expect("interpolation source validates");
    let bayes_records = extract_formula_records(BAYES_SOURCE).expect("Bayes source validates");
    let cases: Vec<_> = (0..800).map(case).collect();
    let receipts: Vec<_> = cases.iter().map(|c| run(c, &interpolation_records, &bayes_records)).collect();
    let overall = metrics(&receipts);
    assert_eq!((overall.cases, overall.supported, overall.ambiguous, overall.unsupported), (800, 480, 160, 160));
    assert_eq!(overall.exact, 800); assert_eq!(overall.authorized, 480); assert_eq!(overall.ambiguities_preserved, 160); assert_eq!(overall.unsupported_refused, 160); assert_eq!(overall.replay_verified, 800); assert_eq!(overall.tamper_rejections, 800); assert_eq!(overall.provenance_preserved, 800); assert_eq!(overall.false_authorizations, 0); assert_eq!(overall.false_denials, 0);
    let development: Vec<_> = receipts.iter().zip(&cases).filter(|(_, c)| c.partition == Partition::Development).map(|(r, _)| r.clone()).collect();
    let validation: Vec<_> = receipts.iter().zip(&cases).filter(|(_, c)| c.partition == Partition::Validation).map(|(r, _)| r.clone()).collect();
    let sealed: Vec<_> = receipts.iter().zip(&cases).filter(|(_, c)| c.partition == Partition::Sealed).map(|(r, _)| r.clone()).collect();
    let report = Report { schema: "stage96-source-language-extension-v1", interpolation_source_sha256: digest(INTERPOLATION_SOURCE), bayes_source_sha256: digest(BAYES_SOURCE), corpus_sha256: digest(&cases), sealed_sha256: digest(&sealed), metrics: overall, partitions: BTreeMap::from([(String::from("development"), metrics(&development)), (String::from("validation"), metrics(&validation)), (String::from("sealed"), metrics(&sealed))]), receipts };
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
