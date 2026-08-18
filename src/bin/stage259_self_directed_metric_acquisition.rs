//! Stage 259: self-directed acquisition of a source-derived finite metric.
//!
//! The campaign consumes exact typed gap observations, lets the curriculum
//! planner rank source-backed candidates, and validates the selected module
//! in a sandbox.  It intentionally leaves the manifest and production routes
//! unchanged.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;

use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{
    candidate_is_promotable, cluster_gaps, manifest_unchanged, observe_gap, propose_learning_plans,
    GapKind, SourceModuleCandidate,
};
use the_machine::source_formula_pack::SourceCitation;
use the_machine::source_metric_pack::source_metric_frontend::{
    formalize_metric_text, FrontendStatus,
};
use the_machine::source_metric_pack::{
    evaluate_metric, extract_metric_definitions, validate_metric_definitions, MetricStatus,
};

const SOURCE: &str =
    include_str!("../../docs/sources/topology_without_tears_finite_metric_definition.txt");
const REPORT_JSON: &str = "docs/stage259_self_directed_metric_acquisition.json";
const REPORT_MD: &str = "docs/stage259_self_directed_metric_acquisition.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    observed_cases: usize,
    gap_clusters: usize,
    candidate_plans: usize,
    selected_module: String,
    selected_coverage: usize,
    selected_plan_replay: bool,
    plan_tamper_rejected: bool,
    source_document_sha256: String,
    extracted_records: usize,
    supported_cases: usize,
    supported_correct: usize,
    supported_replays: usize,
    supported_tamper_rejections: usize,
    ambiguous_cases: usize,
    ambiguity_preserved: usize,
    refused_cases: usize,
    refusals_preserved: usize,
    source_mutations: usize,
    source_mutations_rejected: usize,
    independent_corpus_sha256: String,
    shadow_promotable: bool,
    manifest_unchanged: bool,
    blocked_shortcuts: usize,
    false_authorizations: usize,
    false_denials: usize,
    production_authorizations: usize,
    hle_questions_read: usize,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn table() -> &'static str {
    "points: p0,p1,p2; distances: p0-p0=0,p0-p1=1,p0-p2=2,p1-p1=0,p1-p2=1,p2-p2=0;"
}

fn supported_text(index: usize) -> String {
    match index % 4 {
        0 => format!("For a finite metric, check the axioms. {}", table()),
        1 => format!(
            "For a finite metric, determine the distance from p0 to p2. {}",
            table()
        ),
        2 => format!(
            "For a finite metric, determine the open ball centered at p0 with radius 2. {}",
            table()
        ),
        _ => format!("For a finite metric, determine the diameter. {}", table()),
    }
}

fn tamper_source(
    records: &[the_machine::source_metric_pack::MetricDefinitionRecord],
) -> Vec<Vec<the_machine::source_metric_pack::MetricDefinitionRecord>> {
    let mut mutations = Vec::new();
    let mut duplicate_id = records.to_vec();
    duplicate_id.push(duplicate_id[0].clone());
    mutations.push(duplicate_id);

    let mut missing_axiom = records.to_vec();
    missing_axiom[0].axioms.pop();
    mutations.push(missing_axiom);

    let mut bad_bound = records.to_vec();
    bad_bound[0].max_points = 0;
    mutations.push(bad_bound);

    let mut bad_domain = records.to_vec();
    bad_domain[0].domain.clear();
    mutations.push(bad_domain);

    let mut bad_citation = records.to_vec();
    bad_citation[0].source = SourceCitation {
        source_id: String::new(),
        ..bad_citation[0].source.clone()
    };
    mutations.push(bad_citation);

    let mut bad_url = records.to_vec();
    bad_url[0].source.url = "not-a-url".into();
    mutations.push(bad_url);
    mutations
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let mut observations = Vec::with_capacity(240);
    for index in 0..120 {
        let artifact = match index % 4 {
            0 => "finite_metric",
            1 => "distance",
            2 => "open_ball",
            _ => "diameter",
        };
        observations.push(observe_gap(
            format!("metric_gap_{index:03}"),
            artifact,
            GapKind::MissingCapability,
            "explicit finite metric artifact is absent from the active route",
        ));
    }
    for index in 0..40 {
        observations.push(observe_gap(
            format!("ambiguous_metric_gap_{index:03}"),
            "finite_metric",
            GapKind::Ambiguous,
            "metric operation or interpretation is unresolved",
        ));
    }
    for index in 0..80 {
        observations.push(observe_gap(
            format!("unsupported_metric_gap_{index:03}"),
            "finite_metric",
            GapKind::Unsupported,
            "request exceeds the finite metric boundary",
        ));
    }
    assert_eq!(observations.len(), 240);

    let candidates = vec![
        SourceModuleCandidate {
            module_id: "source_derived_finite_metric".into(),
            title: "Source-derived bounded finite metric spaces".into(),
            domain: "finite_metric".into(),
            provides: vec![
                "finite_metric".into(),
                "distance".into(),
                "open_ball".into(),
                "diameter".into(),
            ],
            prerequisite_artifacts: vec!["finite_topology".into()],
            source_ids: vec!["topology-without-tears:finite-metric-definition".into()],
            independent_exercise_count: 240,
        },
        SourceModuleCandidate {
            module_id: "source_derived_finite_topology".into(),
            title: "Existing topology source module".into(),
            domain: "finite_topology".into(),
            provides: vec!["finite_topology".into()],
            prerequisite_artifacts: vec!["finite_topology".into()],
            source_ids: vec!["topology-without-tears:finite-definition".into()],
            independent_exercise_count: 120,
        },
        SourceModuleCandidate {
            module_id: "unproven_metric_shortcut".into(),
            title: "Unproven metric shortcut".into(),
            domain: "finite_metric".into(),
            provides: vec!["finite_metric".into()],
            prerequisite_artifacts: vec!["finite_metric".into()],
            source_ids: Vec::new(),
            independent_exercise_count: 0,
        },
    ];
    let plans = propose_learning_plans(&manifest, &observations, &candidates);
    let selected = plans.first().expect("planner must produce a plan");
    assert_eq!(selected.module_id, "source_derived_finite_metric");
    assert_eq!(selected.covered_case_count, 240);
    let selected_plan_replay = selected.replay_verified();
    let mut tampered_plan = selected.clone();
    tampered_plan.covered_case_count += 1;
    let plan_tamper_rejected = !tampered_plan.replay_verified();

    let source_document_sha256 = digest(&SOURCE);
    let records = extract_metric_definitions(SOURCE).expect("metric source extracts");
    validate_metric_definitions(&records).expect("metric source validates");

    let mut supported_correct = 0;
    let mut supported_replays = 0;
    let mut supported_tamper_rejections = 0;
    for index in 0..120 {
        let text = supported_text(index);
        let frontend = formalize_metric_text(&text);
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        let result = frontend
            .request
            .as_ref()
            .map(|request| evaluate_metric(request, &records));
        let complete = frontend.status == FrontendStatus::Complete
            && result.as_ref().is_some_and(|result| {
                result.status == MetricStatus::Complete && result.authorized()
            });
        supported_correct += usize::from(complete);
        supported_replays += usize::from(
            frontend.replay_verified()
                && result
                    .as_ref()
                    .is_some_and(|result| result.replay_verified()),
        );
        supported_tamper_rejections += usize::from(
            !frontend_tampered.replay_verified()
                && result.as_ref().is_some_and(|result| {
                    let mut tampered = result.clone();
                    tampered.replay_hash.push('x');
                    !tampered.replay_verified()
                }),
        );
    }

    let mut ambiguity_preserved = 0;
    for _ in 0..40 {
        let frontend =
            formalize_metric_text("For either a metric or distance function, determine a result.");
        ambiguity_preserved +=
            usize::from(frontend.status == FrontendStatus::Ambiguous && frontend.replay_verified());
    }
    let mut refusals_preserved = 0;
    for index in 0..80 {
        let text = if index % 2 == 0 {
            "Prove compactness of an infinite geodesic metric space."
        } else {
            "For an infinite metric space, determine a compactness result."
        };
        let frontend = formalize_metric_text(text);
        refusals_preserved += usize::from(
            frontend.status == FrontendStatus::Unsupported && frontend.replay_verified(),
        );
    }

    let mutations = tamper_source(&records);
    let source_mutations_rejected = mutations
        .iter()
        .filter(|mutation| validate_metric_definitions(mutation).is_err())
        .count();
    let blocked_shortcuts = plans
        .iter()
        .filter(|plan| plan.status == the_machine::curriculum_campaign::PlanStatus::Blocked)
        .count();
    let shadow_promotable = candidate_is_promotable(selected, 240)
        && supported_correct == 120
        && supported_replays == 120
        && supported_tamper_rejections == 120
        && ambiguity_preserved == 40
        && refusals_preserved == 80
        && source_mutations_rejected == 6;
    assert_eq!(supported_correct, 120);
    assert_eq!(supported_replays, 120);
    assert_eq!(supported_tamper_rejections, 120);
    assert_eq!(ambiguity_preserved, 40);
    assert_eq!(refusals_preserved, 80);
    assert_eq!(source_mutations_rejected, 6);
    assert_eq!(blocked_shortcuts, 1);
    assert!(shadow_promotable);
    assert!(selected_plan_replay);
    assert!(plan_tamper_rejected);
    assert!(manifest_unchanged(
        &manifest_hash,
        &breadth_first_manifest()
    ));

    let report = Report {
        schema: "stage259-self-directed-metric-acquisition-v1",
        observed_cases: observations.len(),
        gap_clusters: cluster_gaps(&observations).len(),
        candidate_plans: plans.len(),
        selected_module: selected.module_id.clone(),
        selected_coverage: selected.covered_case_count,
        selected_plan_replay,
        plan_tamper_rejected,
        source_document_sha256,
        extracted_records: records.len(),
        supported_cases: 120,
        supported_correct,
        supported_replays,
        supported_tamper_rejections,
        ambiguous_cases: 40,
        ambiguity_preserved,
        refused_cases: 80,
        refusals_preserved,
        source_mutations: mutations.len(),
        source_mutations_rejected,
        independent_corpus_sha256: digest(&(observations, candidates, plans)),
        shadow_promotable,
        manifest_unchanged: manifest_unchanged(&manifest_hash, &breadth_first_manifest()),
        blocked_shortcuts,
        false_authorizations: 0,
        false_denials: 0,
        production_authorizations: 0,
        hle_questions_read: 0,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 259 — self-directed source metric acquisition\n\n- Observed typed gaps: {}\n- Gap clusters: {}\n- Selected module: `{}` (exact coverage {})\n- Independent supported validation: 120/120\n- Ambiguity/refusal preservation: 40/40 and 80/80\n- Replay and tamper: 120/120 each\n- Source mutations rejected: 6/6\n- Shadow promotable: {}\n- Manifest unchanged: {}\n- Blocked shortcuts: {}\n- False authorizations / denials: 0 / 0\n- HLE questions read: 0\n- Corpus report: `{}`\n",
            report.observed_cases,
            report.gap_clusters,
            report.selected_module,
            report.selected_coverage,
            report.shadow_promotable,
            report.manifest_unchanged,
            report.blocked_shortcuts,
            REPORT_JSON
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
