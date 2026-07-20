//! Audit the formalization report's direct-instantiation predictions.
//!
//! This is a second diagnostic gate.  It classifies the supplied object and
//! requested use, then reports representation readiness.  It never executes a
//! definition, theorem, or solver.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use the_machine::formalization::{
    assess_direct_instantiation, assess_prompt, DirectInstantiationAssessment,
    FalseLowDistanceReason, FormalizationTrace, InstantiationTargetKind, ModelingDistance,
    RepresentationReadiness, SuppliedObjectKind,
};

#[derive(Debug, Deserialize)]
struct HleRow {
    id: String,
    question: String,
    category: String,
    #[serde(default)]
    has_image: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    scanned: usize,
    direct_predictions: usize,
    by_object: BTreeMap<String, usize>,
    by_target: BTreeMap<String, usize>,
    by_readiness: BTreeMap<String, usize>,
    missing_representation: BTreeMap<String, usize>,
    authorization_blockers: BTreeMap<String, usize>,
    false_low_distance_reasons: BTreeMap<String, usize>,
    conservative_lower_bounds: BTreeMap<String, usize>,
    authorization_safe: usize,
    authorization_safe_ids: Vec<String>,
    ids_by_readiness: BTreeMap<String, Vec<String>>,
    ids_by_object: BTreeMap<String, Vec<String>>,
    ids_by_target: BTreeMap<String, Vec<String>>,
    near_ready_reviews: Vec<NearReadyFormalizationReview>,
    assessments: Vec<DirectInstantiationAssessment>,
}

#[derive(Debug, Serialize)]
struct NearReadyFormalizationReview {
    question_id: String,
    category: String,
    question: String,
    supplied_object: String,
    requested_target: String,
    smallest_required_change: String,
    verifier_available_after_change: bool,
    reviewed: bool,
    execution_eligible: bool,
    review_note: String,
}

fn object_label(value: SuppliedObjectKind) -> &'static str {
    value.label()
}

fn target_label(value: InstantiationTargetKind) -> &'static str {
    value.label()
}

fn readiness_label(value: RepresentationReadiness) -> &'static str {
    value.label()
}

fn reason_label(value: FalseLowDistanceReason) -> &'static str {
    value.label()
}

/// Human review of the seven residual near-ready predictions.  This metadata
/// is diagnostic only: it never enables a solver route or supplies a gold
/// answer.
fn review_near_ready(
    assessment: &DirectInstantiationAssessment,
    trace: &FormalizationTrace,
    question: &str,
) -> NearReadyFormalizationReview {
    let (change, verifier, note) = match assessment.question_id.as_str() {
        "66ecf59741de2844089fc54b" => (
            "multiobjective rank-1/Pareto optimization representation",
            false,
            "The matrix/error definition is explicit, but the target asks a specialized Pareto-front theorem, not direct evaluation.",
        ),
        "6724dae7f70a476bbcaa32ef" => (
            "typed lattice invariants and farness/neighbor theorem support",
            false,
            "The lattice subquestions require specialized definitions and independent theorem verification.",
        ),
        "6725716480b9caf2f8f62d01" => (
            "infinite-grid combinatorial modeling and proof/search",
            false,
            "The repeated grid-neighborhood wording hides existence, asymptotic, and extremal claims rather than one rule application.",
        ),
        "672d9a18a3ca2744fbeb434f" => (
            "algorithmic memory-model and prime-sieve construction",
            false,
            "This asks for an optimal data-structure design and lower-bound size, not verification of a supplied algorithm.",
        ),
        "67381b2862660a32c77bfe3d" => (
            "security-domain statement validation and multi-select evidence",
            false,
            "The attack-graph prose supplies claims but no executable rule; correctness depends on domain knowledge and choice verification.",
        ),
        "673a85e1551b8b9cc471012d" => (
            "bounded integer factorization and primality predicate",
            true,
            "This is the closest candidate, but no authorized number-theory method currently supports factorization/primality.",
        ),
        "66eddc58fcc3c877643b5f39" => (
            "nonlinear ODE boundary-value/numerical solver",
            false,
            "The equation and one boundary value do not authorize direct evaluation at x=0; nonlinear boundary-value analysis is required.",
        ),
        _ => (
            "manual domain-specific review",
            false,
            "No generic execution route is enabled for this diagnostic.",
        ),
    };
    NearReadyFormalizationReview {
        question_id: assessment.question_id.clone(),
        category: trace.category.clone(),
        question: question.into(),
        supplied_object: assessment.supplied_object.label().to_string(),
        requested_target: assessment.target.label().to_string(),
        smallest_required_change: change.into(),
        verifier_available_after_change: verifier,
        reviewed: true,
        execution_eligible: false,
        review_note: note.into(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let text = fs::read_to_string("data/hle.jsonl")?;
    let mut traces: Vec<FormalizationTrace> = Vec::new();
    let mut questions_by_id = BTreeMap::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let row: HleRow = serde_json::from_str(line)?;
        traces.push(assess_prompt(
            &row.id,
            &row.question,
            &row.category,
            row.has_image,
        ));
        questions_by_id.insert(row.id, row.question);
    }
    let candidates = traces
        .iter()
        .filter(|trace| trace.modeling_distance == ModelingDistance::DirectInstantiation)
        .map(assess_direct_instantiation)
        .collect::<Vec<_>>();
    let mut by_object = BTreeMap::new();
    let mut by_target = BTreeMap::new();
    let mut by_readiness = BTreeMap::new();
    let mut missing_representation = BTreeMap::new();
    let mut authorization_blockers = BTreeMap::new();
    let mut false_low_distance_reasons = BTreeMap::new();
    let mut conservative_lower_bounds = BTreeMap::new();
    let mut authorization_safe_ids = Vec::new();
    let mut ids_by_readiness: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut ids_by_object: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut ids_by_target: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for assessment in &candidates {
        let object = object_label(assessment.supplied_object).to_string();
        let target = target_label(assessment.target).to_string();
        let readiness = readiness_label(assessment.readiness).to_string();
        *by_object.entry(object.clone()).or_insert(0) += 1;
        *by_target.entry(target.clone()).or_insert(0) += 1;
        *by_readiness.entry(readiness.clone()).or_insert(0) += 1;
        *conservative_lower_bounds
            .entry(assessment.conservative_lower_bound.label().to_string())
            .or_insert(0) += 1;
        ids_by_object
            .entry(object)
            .or_default()
            .push(assessment.question_id.clone());
        ids_by_target
            .entry(target)
            .or_default()
            .push(assessment.question_id.clone());
        ids_by_readiness
            .entry(readiness)
            .or_default()
            .push(assessment.question_id.clone());
        for missing in &assessment.missing_representation {
            *missing_representation.entry(missing.clone()).or_insert(0) += 1;
        }
        for blocker in &assessment.authorization_blockers {
            *authorization_blockers.entry(blocker.clone()).or_insert(0) += 1;
        }
        if let Some(reason) = assessment.false_low_distance_reason {
            *false_low_distance_reasons
                .entry(reason_label(reason).to_string())
                .or_insert(0) += 1;
        }
        if assessment.authorization_safe() {
            authorization_safe_ids.push(assessment.question_id.clone());
        }
    }
    let near_ready_reviews = candidates
        .iter()
        .filter(|assessment| assessment.readiness == RepresentationReadiness::NearReady)
        .filter_map(|assessment| {
            traces
                .iter()
                .find(|trace| trace.question_id == assessment.question_id)
                .and_then(|trace| {
                    questions_by_id
                        .get(&trace.question_id)
                        .map(|question| review_near_ready(assessment, trace, question))
                })
        })
        .collect::<Vec<_>>();
    let report = Report {
        scanned: traces.len(),
        direct_predictions: candidates.len(),
        by_object,
        by_target,
        by_readiness,
        missing_representation,
        authorization_blockers,
        false_low_distance_reasons,
        conservative_lower_bounds,
        authorization_safe: authorization_safe_ids.len(),
        authorization_safe_ids,
        ids_by_readiness,
        ids_by_object,
        ids_by_target,
        near_ready_reviews,
        assessments: candidates,
    };
    fs::write(
        "docs/direct_instantiation_audit_20260720.json",
        serde_json::to_string_pretty(&report)?,
    )?;
    let mut markdown = String::from(
        "# Direct-instantiation audit\n\nThis report is diagnostic only. It does not authorize definition application, theorem use, or solver execution.\n\n",
    );
    markdown.push_str(&format!(
        "Scanned **{}** questions; **{}** were classified as direct-instantiation-shaped.\n\n",
        report.scanned, report.direct_predictions
    ));
    markdown.push_str("## Representation readiness\n\n| Readiness | Questions |\n|---|---:|\n");
    for (key, value) in &report.by_readiness {
        markdown.push_str(&format!("| {} | {} |\n", key, value));
    }
    markdown.push_str("\n## Supplied objects\n\n| Object | Questions |\n|---|---:|\n");
    for (key, value) in &report.by_object {
        markdown.push_str(&format!("| {} | {} |\n", key, value));
    }
    markdown.push_str("\n## Requested uses\n\n| Target | Questions |\n|---|---:|\n");
    for (key, value) in &report.by_target {
        markdown.push_str(&format!("| {} | {} |\n", key, value));
    }
    markdown.push_str("\n## Missing representation\n\n| Gap | Questions |\n|---|---:|\n");
    for (key, value) in &report.missing_representation {
        markdown.push_str(&format!("| {} | {} |\n", key, value));
    }
    markdown.push_str("\n## Authorization blockers\n\n| Blocker | Questions |\n|---|---:|\n");
    for (key, value) in &report.authorization_blockers {
        markdown.push_str(&format!("| {} | {} |\n", key, value));
    }
    markdown.push_str("\n## False low-distance reasons\n\n| Reason | Questions |\n|---|---:|\n");
    for (key, value) in &report.false_low_distance_reasons {
        markdown.push_str(&format!("| {} | {} |\n", key, value));
    }
    markdown.push_str(
        "\n## Conservative lower bounds\n\n| Minimum distance | Questions |\n|---|---:|\n",
    );
    for (key, value) in &report.conservative_lower_bounds {
        markdown.push_str(&format!("| {} | {} |\n", key, value));
    }
    markdown.push_str(&format!(
        "\n**Authorization-safe assessments:** {} (IDs: {}).\n\n",
        report.authorization_safe,
        if report.authorization_safe_ids.is_empty() {
            "none".to_string()
        } else {
            report.authorization_safe_ids.join(", ")
        }
    ));
    markdown.push_str("\n## Manual review of near-ready cases\n\n");
    for review in &report.near_ready_reviews {
        markdown.push_str(&format!(
            "- **{}** — `{}` → `{}`; change: {}; verifier after change: {}; execution eligible: {}. {}\n",
            review.question_id,
            review.supplied_object,
            review.requested_target,
            review.smallest_required_change,
            review.verifier_available_after_change,
            review.execution_eligible,
            review.review_note
        ));
    }
    markdown.push_str("\n## Supporting question IDs by readiness\n\n");
    for (key, ids) in &report.ids_by_readiness {
        markdown.push_str(&format!(
            "- **{}** ({}): {}\n",
            key,
            ids.len(),
            ids.join(", ")
        ));
    }
    markdown.push_str(
        "\nThe JSON report also contains exact IDs for every supplied-object and target bucket.\n",
    );
    markdown.push_str("\nEvery successful-looking assessment still requires manual review and an independent verifier before any runtime route can be enabled.\n");
    fs::write("docs/direct_instantiation_audit_20260720.md", markdown)?;
    println!(
        "scanned={} direct_predictions={} readiness={:?} objects={:?} targets={:?}",
        report.scanned,
        report.direct_predictions,
        report.by_readiness,
        report.by_object,
        report.by_target
    );
    Ok(())
}
